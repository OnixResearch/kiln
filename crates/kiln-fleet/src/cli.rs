//! Command line. Exit 0 with JSON on stdout; 1 for refusals, conflicts and
//! HTTP errors with JSON on stderr; 2 for usage errors.

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::api::Service;
use crate::client::{ApiClient, SERVER_ENVIRONMENT};
use crate::config::{CONSOLE_URL_ENVIRONMENT, FleetConfig, validate_console_url};
use crate::dispatch::Dispatcher;
use crate::mcp::McpServer;
use crate::{log, server};

pub const EXIT_OK: i32 = 0;
pub const EXIT_REFUSED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

pub const USAGE: &str = "usage:
  kiln-fleet run --config FILE [--console-url URL]
  kiln-fleet check-config --config FILE
  kiln-fleet mcp [--server URL] [--tool NAME]
  kiln-fleet claim --scope '<target> <path>' --tool T --session S --purpose TEXT [--lease-minutes N]
  kiln-fleet hold --scope '<target> <path>' --tool T --session S --purpose TEXT [--lease-minutes N] [--server URL] -- <command> [args...]
  kiln-fleet renew --claim ID --fence N [--lease-minutes N]
  kiln-fleet release --claim ID --fence N [--note TEXT]
  kiln-fleet claims | work | attempts | decisions
  kiln-fleet status [--format json|waybar|tmux] [--server URL]
  kiln-fleet watch [--interval SECONDS] [--iterations N] [--server URL]
  kiln-fleet handoff --attempt ID --fence N --commit SHA
  kiln-fleet answer --decision ID --text TEXT
  kiln-fleet cancel --attempt ID --reason TEXT
  kiln-fleet swarm list | status | create | spawn | member-state | post | read | verdict | watchdog | failover | graph ...
client commands take --server URL (default $KILN_FLEET_SERVER)";

struct Usage(String);

type Flags = BTreeMap<String, String>;

/// Run the command line; returns the process exit code.
#[must_use]
pub fn main_with(arguments: &[String]) -> i32 {
    let Some((command, rest)) = arguments.split_first() else {
        return usage("missing command");
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return EXIT_OK;
    }
    let outcome = match command.as_str() {
        "run" => flags(rest, &["--config"], &["--console-url"]).map(|flags| run(&flags)),
        "check-config" => flags(rest, &["--config"], &[]).map(|flags| check_config(&flags)),
        "mcp" => flags(rest, &[], &["--server", "--tool"]).and_then(|flags| mcp(&flags)),
        "claim" => flags(
            rest,
            &["--scope", "--tool", "--session", "--purpose"],
            &["--lease-minutes", "--server"],
        )
        .and_then(|flags| {
            let mut body = json!({
                "scope": required(&flags, "--scope")?,
                "tool": required(&flags, "--tool")?,
                "session": required(&flags, "--session")?,
                "purpose": required(&flags, "--purpose")?,
            });
            insert_number(&mut body, &flags, "--lease-minutes", "lease_minutes")?;
            call(&flags, "POST", "/v1/claims".to_owned(), Some(body))
        }),
        "renew" => flags(rest, &["--claim", "--fence"], &["--lease-minutes", "--server"]).and_then(|flags| {
            let mut body = json!({ "fence": number(&flags, "--fence")? });
            insert_number(&mut body, &flags, "--lease-minutes", "lease_minutes")?;
            let path = format!("/v1/claims/{}/renew", identity(&flags, "--claim")?);
            call(&flags, "POST", path, Some(body))
        }),
        "release" => flags(rest, &["--claim", "--fence"], &["--note", "--server"]).and_then(|flags| {
            let mut body = json!({ "fence": number(&flags, "--fence")? });
            if let (Some(note), Some(object)) = (flags.get("--note"), body.as_object_mut()) {
                object.insert("note".into(), json!(note));
            }
            let path = format!("/v1/claims/{}/release", identity(&flags, "--claim")?);
            call(&flags, "POST", path, Some(body))
        }),
        "claims" | "work" | "attempts" | "decisions" => {
            flags(rest, &[], &["--server"]).and_then(|flags| call(&flags, "GET", format!("/v1/{command}"), None))
        }
        "status" => flags(rest, &[], &["--format", "--server"]).and_then(|flags| status(&flags)),
        "watch" => flags(rest, &[], &["--interval", "--iterations", "--server"])
            .and_then(|flags| watch(&flags)),
        "handoff" => flags(rest, &["--attempt", "--fence", "--commit"], &["--server"]).and_then(|flags| {
            let body = json!({ "fence": number(&flags, "--fence")?, "commit": required(&flags, "--commit")? });
            let path = format!("/v1/attempts/{}/handoff", identity(&flags, "--attempt")?);
            call(&flags, "POST", path, Some(body))
        }),
        "answer" => flags(rest, &["--decision", "--text"], &["--server"]).and_then(|flags| {
            let body = json!({ "text": required(&flags, "--text")? });
            let path = format!("/v1/decisions/{}/answer", identity(&flags, "--decision")?);
            call(&flags, "POST", path, Some(body))
        }),
        "cancel" => flags(rest, &["--attempt", "--reason"], &["--server"]).and_then(|flags| {
            let body = json!({ "reason": required(&flags, "--reason")? });
            let path = format!("/v1/attempts/{}/cancel", identity(&flags, "--attempt")?);
            call(&flags, "POST", path, Some(body))
        }),
        "hold" => hold_command(rest),
        "swarm" => swarm_command(rest),
        other => Err(Usage(format!("unknown command {other}"))),
    };
    outcome.unwrap_or_else(|Usage(message)| usage(&message))
}

fn usage(message: &str) -> i32 {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{}", json!({ "error": message, "usage": USAGE }));
    EXIT_USAGE
}

fn hold_command(arguments: &[String]) -> Result<i32, Usage> {
    let Some(separator) = arguments.iter().position(|argument| argument == "--") else {
        return Err(Usage("hold requires -- <command> [args...]".into()));
    };
    let flag_args = arguments
        .get(..separator)
        .ok_or_else(|| Usage("hold requires -- <command> [args...]".into()))?;
    let command_args = arguments
        .get(separator.saturating_add(1)..)
        .ok_or_else(|| Usage("hold requires a command after --".into()))?;
    if command_args.is_empty() {
        return Err(Usage("hold requires a command after --".into()));
    }
    let flags = flags(
        flag_args,
        &["--scope", "--tool", "--session", "--purpose"],
        &["--lease-minutes", "--server"],
    )?;
    let lease_minutes = if flags.contains_key("--lease-minutes") {
        let minutes = number(&flags, "--lease-minutes")?;
        if !(crate::hold::MIN_HOLD_LEASE_MINUTES..=crate::hold::MAX_HOLD_LEASE_MINUTES)
            .contains(&minutes)
        {
            return Err(Usage(format!(
                "--lease-minutes must be between {} and {}",
                crate::hold::MIN_HOLD_LEASE_MINUTES,
                crate::hold::MAX_HOLD_LEASE_MINUTES
            )));
        }
        minutes
    } else {
        crate::hold::DEFAULT_HOLD_LEASE_MINUTES
    };
    let client = server(&flags)?;
    let spec = crate::hold::HoldSpec {
        scope: required(&flags, "--scope")?,
        tool: required(&flags, "--tool")?,
        session: required(&flags, "--session")?,
        purpose: required(&flags, "--purpose")?,
        lease_minutes,
        command: command_args.to_vec(),
    };
    Ok(crate::hold::run(&client, &spec))
}

// r[impl kiln.fleet.swarm.surface]
fn swarm_command(arguments: &[String]) -> Result<i32, Usage> {
    let Some((sub, rest)) = arguments.split_first() else {
        return Err(Usage("swarm requires a subcommand".into()));
    };
    match sub.as_str() {
        "list" => {
            let flags = flags(rest, &[], &["--server"])?;
            call(&flags, "GET", "/v1/swarms".to_owned(), None)
        }
        "status" => {
            let flags = flags(rest, &["--swarm"], &["--server"])?;
            let path = format!("/v1/swarms/{}", identity(&flags, "--swarm")?);
            call(&flags, "GET", path, None)
        }
        "create" => {
            let flags = flags(
                rest,
                &["--title", "--scope"],
                &[
                    "--max-concurrency",
                    "--max-repair-rounds",
                    "--budget",
                    "--stale-after-ms",
                    "--server",
                ],
            )?;
            let scopes: Vec<String> = required(&flags, "--scope")?
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            let mut body = json!({
                "title": required(&flags, "--title")?,
                "allowed_scopes": scopes,
            });
            insert_number(&mut body, &flags, "--max-concurrency", "max_concurrency")?;
            insert_number(
                &mut body,
                &flags,
                "--max-repair-rounds",
                "max_repair_rounds",
            )?;
            insert_number(&mut body, &flags, "--budget", "budget_ceiling_units")?;
            insert_number(&mut body, &flags, "--stale-after-ms", "stale_after_ms")?;
            call(&flags, "POST", "/v1/swarms".to_owned(), Some(body))
        }
        "spawn" => {
            let flags = flags(
                rest,
                &[
                    "--swarm",
                    "--name",
                    "--role",
                    "--harness",
                    "--model",
                    "--brief",
                ],
                &[
                    "--host",
                    "--scope",
                    "--requester",
                    "--attempt",
                    "--claim",
                    "--fence",
                    "--server",
                ],
            )?;
            let mut body = json!({
                "name": required(&flags, "--name")?,
                "role": required(&flags, "--role")?,
                "harness": required(&flags, "--harness")?,
                "model": required(&flags, "--model")?,
                "brief": required(&flags, "--brief")?,
            });
            if let Some(object) = body.as_object_mut() {
                if let Some(host) = flags.get("--host") {
                    object.insert("host".into(), json!(host));
                }
                if let Some(scope) = flags.get("--scope") {
                    object.insert("scope".into(), json!(scope));
                }
                if let Some(requester) = flags.get("--requester") {
                    object.insert("requester".into(), json!(requester));
                }
                if flags.contains_key("--attempt") {
                    object.insert("attempt".into(), json!(identity(&flags, "--attempt")?));
                }
                if flags.contains_key("--claim") {
                    object.insert("claim_identity".into(), json!(identity(&flags, "--claim")?));
                }
            }
            insert_number(&mut body, &flags, "--fence", "claim_fence")?;
            let path = format!("/v1/swarms/{}/members", identity(&flags, "--swarm")?);
            call(&flags, "POST", path, Some(body))
        }
        "member-state" => {
            let flags = flags(
                rest,
                &["--swarm", "--member", "--state"],
                &[
                    "--since-seq",
                    "--checkpoint",
                    "--board-cursor",
                    "--reason",
                    "--paused-ms",
                    "--budget-draw",
                    "--server",
                ],
            )?;
            let state_name = required(&flags, "--state")?;
            let state_json = match state_name.as_str() {
                "working" | "retired" | "terminal" => json!({ "state": state_name }),
                "waiting" => json!({
                    "state": "waiting",
                    "since_seq": number(&flags, "--since-seq")?,
                }),
                "paused_quota" => json!({
                    "state": "paused_quota",
                    "checkpoint_commit": flags.get("--checkpoint"),
                    "board_cursor": number(&flags, "--board-cursor")?,
                    "reason": required(&flags, "--reason")?,
                    "paused_ms": flags
                        .get("--paused-ms")
                        .map_or(Ok(crate::log::now_ms()), |v| v.parse::<u64>())
                        .map_err(|_| Usage("--paused-ms must be a non-negative integer".into()))?,
                }),
                other => {
                    return Err(Usage(format!(
                        "unknown member state {other}; expected working, waiting, paused_quota, retired, or terminal"
                    )));
                }
            };
            let mut body = json!({
                "action": "update_state",
                "member": required(&flags, "--member")?,
                "state": state_json,
            });
            insert_number(&mut body, &flags, "--budget-draw", "budget_draw_delta")?;
            let path = format!("/v1/swarms/{}/members", identity(&flags, "--swarm")?);
            call(&flags, "POST", path, Some(body))
        }
        "post" => {
            let flags = flags(
                rest,
                &["--swarm", "--sender", "--kind", "--body"],
                &["--mentions", "--reply-to", "--commit", "--server"],
            )?;
            let mentions: Vec<Value> = flags
                .get("--mentions")
                .map(|raw| {
                    raw.split(',')
                        .map(str::trim)
                        .filter(|token| !token.is_empty())
                        .map(parse_mention_cli)
                        .collect()
                })
                .unwrap_or_default();
            let mut body = json!({
                "sender": required(&flags, "--sender")?,
                "kind": required(&flags, "--kind")?,
                "mentions": mentions,
                "body": required(&flags, "--body")?,
            });
            insert_number(&mut body, &flags, "--reply-to", "in_reply_to")?;
            if let (Some(commit), Some(object)) = (flags.get("--commit"), body.as_object_mut()) {
                object.insert("candidate_commit".into(), json!(commit));
            }
            let path = format!("/v1/swarms/{}/board", identity(&flags, "--swarm")?);
            call(&flags, "POST", path, Some(body))
        }
        "read" => {
            let flags = flags(
                rest,
                &["--swarm"],
                &["--since", "--limit", "--seqs", "--member", "--server"],
            )?;
            let mut query = Vec::new();
            if flags.contains_key("--since") {
                query.push(format!("since={}", number(&flags, "--since")?));
            }
            if flags.contains_key("--limit") {
                query.push(format!("limit={}", number(&flags, "--limit")?));
            }
            if let Some(seqs) = flags.get("--seqs") {
                let mut validated = Vec::new();
                for token in seqs.split(',') {
                    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(Usage(
                            "--seqs must be a comma-separated list of positive integers".into(),
                        ));
                    }
                    let parsed = token.parse::<u64>().map_err(|_| {
                        Usage("--seqs must be a comma-separated list of positive integers".into())
                    })?;
                    validated.push(parsed.to_string());
                }
                if validated.is_empty() {
                    return Err(Usage(
                        "--seqs must be a comma-separated list of positive integers".into(),
                    ));
                }
                query.push(format!("seqs={}", validated.join(",")));
            }
            if let Some(member) = flags.get("--member") {
                let mut bytes = member.bytes();
                let valid = !member.is_empty()
                    && member.len() <= kiln_core::fleet::MAX_MEMBER_NAME_BYTES
                    && bytes
                        .next()
                        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
                    && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
                if !valid {
                    return Err(Usage(
                        "--member must match [a-z0-9][a-z0-9-]* within 64 bytes".into(),
                    ));
                }
                query.push(format!("member={member}"));
            }
            let base = format!("/v1/swarms/{}/board", identity(&flags, "--swarm")?);
            let path = if query.is_empty() {
                base
            } else {
                format!("{base}?{}", query.join("&"))
            };
            call(&flags, "GET", path, None)
        }
        "verdict" => {
            let flags = flags(
                rest,
                &[
                    "--swarm",
                    "--verifier",
                    "--author",
                    "--commit",
                    "--decision",
                ],
                &["--findings", "--reason", "--server"],
            )?;
            let mut body = json!({
                "verifier": required(&flags, "--verifier")?,
                "author": required(&flags, "--author")?,
                "commit": required(&flags, "--commit")?,
                "verdict": required(&flags, "--decision")?,
            });
            if let Some(object) = body.as_object_mut() {
                if let Some(findings) = flags.get("--findings") {
                    let list: Vec<String> = findings
                        .split(';')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    object.insert("findings".into(), json!(list));
                }
                if let Some(reason) = flags.get("--reason") {
                    object.insert("reason".into(), json!(reason));
                }
            }
            let path = format!("/v1/swarms/{}/verdicts", identity(&flags, "--swarm")?);
            call(&flags, "POST", path, Some(body))
        }
        "watchdog" => {
            let flags = flags(rest, &["--swarm"], &["--apply", "--server"])?;
            let apply = match flags.get("--apply").map(String::as_str) {
                None | Some("true") => true,
                Some("false") => false,
                Some(_) => return Err(Usage("--apply must be true or false".into())),
            };
            let path = format!("/v1/swarms/{}/watchdog", identity(&flags, "--swarm")?);
            call(&flags, "POST", path, Some(json!({ "apply": apply })))
        }
        "failover" => {
            let flags = flags(
                rest,
                &["--swarm", "--member", "--model"],
                &["--harness", "--host", "--reason", "--server"],
            )?;
            let mut body = json!({
                "member": required(&flags, "--member")?,
                "replacement_model": required(&flags, "--model")?,
            });
            if let Some(object) = body.as_object_mut() {
                if let Some(harness) = flags.get("--harness") {
                    object.insert("replacement_harness".into(), json!(harness));
                }
                if let Some(host) = flags.get("--host") {
                    object.insert("replacement_host".into(), json!(host));
                }
                if let Some(reason) = flags.get("--reason") {
                    object.insert("reason".into(), json!(reason));
                }
            }
            let path = format!("/v1/swarms/{}/failover", identity(&flags, "--swarm")?);
            call(&flags, "POST", path, Some(body))
        }
        "graph" => {
            let flags = flags(rest, &["--swarm"], &["--up-to-seq", "--server"])?;
            let base = format!("/v1/swarms/{}/graph", identity(&flags, "--swarm")?);
            let path = if flags.contains_key("--up-to-seq") {
                format!("{base}?up_to_seq={}", number(&flags, "--up-to-seq")?)
            } else {
                base
            };
            call(&flags, "GET", path, None)
        }
        other => Err(Usage(format!("unknown swarm subcommand {other}"))),
    }
}

fn parse_mention_cli(token: &str) -> Value {
    let trimmed = token.strip_prefix('@').unwrap_or(token);
    if trimmed == "all" {
        json!({ "kind": "all" })
    } else if let Some(role) = trimmed.strip_prefix("role:") {
        json!({ "kind": "role", "target": role })
    } else {
        json!({ "kind": "member", "target": trimmed })
    }
}

fn refused(value: &Value) -> i32 {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{value}");
    EXIT_REFUSED
}

fn flags(arguments: &[String], required_flags: &[&str], optional: &[&str]) -> Result<Flags, Usage> {
    let mut parsed = Flags::new();
    let mut iterator = arguments.iter();
    while let Some(name) = iterator.next() {
        if !required_flags.contains(&name.as_str()) && !optional.contains(&name.as_str()) {
            return Err(Usage(format!("unknown argument {name}")));
        }
        let value = iterator
            .next()
            .ok_or_else(|| Usage(format!("{name} needs a value")))?;
        if parsed.insert(name.clone(), value.clone()).is_some() {
            return Err(Usage(format!("{name} is repeated")));
        }
    }
    for name in required_flags {
        if !parsed.contains_key(*name) {
            return Err(Usage(format!("{name} is required")));
        }
    }
    Ok(parsed)
}

fn required(flags: &Flags, name: &str) -> Result<String, Usage> {
    flags
        .get(name)
        .cloned()
        .ok_or_else(|| Usage(format!("{name} is required")))
}

fn number(flags: &Flags, name: &str) -> Result<u64, Usage> {
    required(flags, name)?
        .parse::<u64>()
        .map_err(|_| Usage(format!("{name} must be a non-negative integer")))
}

fn insert_number(body: &mut Value, flags: &Flags, name: &str, field: &str) -> Result<(), Usage> {
    if flags.contains_key(name) {
        let value = number(flags, name)?;
        if let Some(object) = body.as_object_mut() {
            object.insert(field.into(), json!(value));
        }
    }
    Ok(())
}

fn identity(flags: &Flags, name: &str) -> Result<String, Usage> {
    let value = required(flags, name)?;
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value)
    } else {
        Err(Usage(format!(
            "{name} must be a 64-character lowercase hex id"
        )))
    }
}

fn server(flags: &Flags) -> Result<ApiClient, Usage> {
    let server = flags
        .get("--server")
        .cloned()
        .or_else(|| std::env::var(SERVER_ENVIRONMENT).ok())
        .ok_or_else(|| Usage(format!("--server or {SERVER_ENVIRONMENT} is required")))?;
    ApiClient::new(&server).map_err(Usage)
}

fn call(flags: &Flags, method: &str, path: String, body: Option<Value>) -> Result<i32, Usage> {
    let client = server(flags)?;
    Ok(match client.request(method, &path, body.as_ref()) {
        Ok((status, value)) if (200..300).contains(&status) => {
            println!("{value}");
            EXIT_OK
        }
        Ok((status, value)) => refused(&json!({ "status": status, "response": value })),
        Err(error) => refused(&json!({ "error": error })),
    })
}

fn endpoint(client: &ApiClient, path: &str) -> Result<Value, String> {
    match client.request("GET", path, None) {
        Ok((status, value)) if (200..300).contains(&status) => Ok(value),
        Ok((status, value)) => Err(format!("{path} returned {status}: {value}")),
        Err(error) => Err(error),
    }
}

fn overview(client: &ApiClient) -> Result<Value, String> {
    Ok(json!({
        "health": endpoint(client, "/v1/health")?,
        "claims": endpoint(client, "/v1/claims")?,
        "work": endpoint(client, "/v1/work")?,
        "attempts": endpoint(client, "/v1/attempts")?,
        "decisions": endpoint(client, "/v1/decisions")?,
    }))
}

fn array_len(value: &Value, pointer: &str) -> usize {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map_or(0, Vec::len)
}

fn open_decisions(value: &Value) -> usize {
    value
        .pointer("/decisions")
        .and_then(Value::as_array)
        .map_or(0, |rows| {
            rows.iter()
                .filter(|row| {
                    row.get("answer").is_none_or(Value::is_null)
                        && row.get("closed_ms").is_none_or(Value::is_null)
                })
                .count()
        })
}

fn counts(snapshot: &Value) -> (usize, usize, usize, usize) {
    (
        array_len(snapshot, "/claims/claims"),
        snapshot
            .pointer("/work/ready_unclaimed")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        array_len(snapshot, "/attempts/attempts"),
        open_decisions(snapshot),
    )
}

fn status(flags: &Flags) -> Result<i32, Usage> {
    let client = server(flags)?;
    let format = flags.get("--format").map_or("json", String::as_str);
    if !matches!(format, "json" | "waybar" | "tmux") {
        return Err(Usage("--format must be json, waybar or tmux".into()));
    }
    let snapshot = overview(&client).map_err(Usage)?;
    let (claims, ready, attempts, decisions) = counts(&snapshot);
    let machine = snapshot
        .pointer("/health/machine")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    match format {
        "waybar" => println!(
            "{}",
            json!({
                "text": format!("Fleet C{claims} R{ready} A{attempts} D{decisions}"),
                "alt": "fleet",
                "class": if decisions > 0 { "fleet-attention" } else { "fleet" },
                "tooltip": format!("{machine}: {claims} claims, {ready} ready work, {attempts} attempts, {decisions} open decisions"),
            })
        ),
        "tmux" => println!("Fleet C:{claims} R:{ready} A:{attempts} D:{decisions}"),
        _ => println!(
            "{}",
            json!({
                "machine": machine,
                "claims": claims,
                "ready_work": ready,
                "attempts": attempts,
                "open_decisions": decisions,
            })
        ),
    }
    Ok(EXIT_OK)
}

fn scope_label(scope: &Value) -> String {
    let target = scope
        .pointer("/target/repository")
        .or_else(|| scope.pointer("/target/machine"))
        .or_else(|| scope.get("target"))
        .and_then(Value::as_str)
        .unwrap_or("?");
    let path = scope
        .pointer("/path/path")
        .or_else(|| scope.get("path"))
        .and_then(Value::as_str)
        .unwrap_or("whole");
    if path == "whole" {
        target.to_owned()
    } else {
        format!("{target} {path}")
    }
}

fn label(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn dashboard(snapshot: &Value) -> String {
    let (claims, ready, attempts, decisions) = counts(snapshot);
    let machine = snapshot
        .pointer("/health/machine")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut text = format!(
        "Fleet {machine}\nclaims {claims}  ready {ready}  attempts {attempts}  open decisions {decisions}\n\nClaims\n"
    );
    if let Some(rows) = snapshot.pointer("/claims/claims").and_then(Value::as_array) {
        for row in rows {
            let identity = row.get("identity").and_then(Value::as_str).unwrap_or("");
            let holder = row
                .pointer("/holder/tool")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let purpose = row.get("purpose").and_then(Value::as_str).unwrap_or("");
            text.push_str(&format!(
                "  {}  {:<40} {}  {}\n",
                identity.chars().take(12).collect::<String>(),
                scope_label(row.get("scope").unwrap_or(&Value::Null)),
                holder,
                purpose.chars().take(60).collect::<String>(),
            ));
        }
    }
    text.push_str("\nAttempts\n");
    if let Some(rows) = snapshot
        .pointer("/attempts/attempts")
        .and_then(Value::as_array)
    {
        for row in rows {
            let identity = row.get("attempt").and_then(Value::as_str).unwrap_or("");
            let state = row.get("state").cloned().unwrap_or(Value::Null);
            let repository = row.get("repository").and_then(Value::as_str).unwrap_or("");
            text.push_str(&format!(
                "  {}  {:<24} {}\n",
                identity.chars().take(12).collect::<String>(),
                label(&state).chars().take(24).collect::<String>(),
                repository,
            ));
        }
    }
    text
}

fn watch_command(client: &ApiClient, line: &str) -> String {
    let mut parts = line.splitn(4, ' ');
    let Some(command) = parts.next() else {
        return "No command.".into();
    };
    let identity = parts.next().unwrap_or_default();
    let valid_identity = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    let request = match command {
        "help" => {
            return "renew ID FENCE [MINUTES] | release ID FENCE [NOTE] | answer ID TEXT | cancel ID REASON | quit"
                .into()
        }
        "renew" => {
            let Some(fence) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
                return "renew needs ID FENCE [MINUTES]".into();
            };
            if !valid_identity(identity) {
                return "renew needs a 64-character lowercase hex claim id".into();
            }
            let mut body = json!({ "fence": fence });
            if let Some(minutes) = parts.next().and_then(|value| value.parse::<u64>().ok())
                && let Some(object) = body.as_object_mut() {
                    object.insert("lease_minutes".into(), json!(minutes));
                }
            (
                "POST",
                format!("/v1/claims/{identity}/renew"),
                Some(body),
            )
        }
        "release" => {
            let Some(fence) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
                return "release needs ID FENCE [NOTE]".into();
            };
            if !valid_identity(identity) {
                return "release needs a 64-character lowercase hex claim id".into();
            }
            let mut body = json!({ "fence": fence });
            if let Some(note) = parts.next().filter(|note| !note.trim().is_empty())
                && let Some(object) = body.as_object_mut() {
                    object.insert("note".into(), json!(note));
                }
            (
                "POST",
                format!("/v1/claims/{identity}/release"),
                Some(body),
            )
        }
        "answer" => {
            let Some(text) = parts.next().filter(|text| !text.trim().is_empty()) else {
                return "answer needs ID TEXT".into();
            };
            if !valid_identity(identity) {
                return "answer needs a 64-character lowercase hex decision id".into();
            }
            (
                "POST",
                format!("/v1/decisions/{identity}/answer"),
                Some(json!({ "text": text })),
            )
        }
        "cancel" => {
            let Some(reason) = parts.next().filter(|reason| !reason.trim().is_empty()) else {
                return "cancel needs ID REASON".into();
            };
            if !valid_identity(identity) {
                return "cancel needs a 64-character lowercase hex attempt id".into();
            }
            (
                "POST",
                format!("/v1/attempts/{identity}/cancel"),
                Some(json!({ "reason": reason })),
            )
        }
        other => return format!("Unknown command {other}. Type help."),
    };
    match client.request(request.0, &request.1, request.2.as_ref()) {
        Ok((status, value)) => format!("{status} {value}"),
        Err(error) => error,
    }
}

fn watch(flags: &Flags) -> Result<i32, Usage> {
    let client = server(flags)?;
    let interval = flags
        .get("--interval")
        .map_or(Ok(5_u64), |value| value.parse::<u64>())
        .map_err(|_| Usage("--interval must be a positive integer".into()))?;
    if interval == 0 || interval > 3_600 {
        return Err(Usage(
            "--interval must be between 1 and 3600 seconds".into(),
        ));
    }
    let iterations = flags
        .get("--iterations")
        .map_or(Ok(0_u64), |value| value.parse::<u64>())
        .map_err(|_| Usage("--iterations must be a non-negative integer".into()))?;
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let mut seen = 0_u64;
    let mut notice = String::new();
    loop {
        let body = match overview(&client) {
            Ok(snapshot) => dashboard(&snapshot),
            Err(error) => format!("Fleet unavailable: {error}\n"),
        };
        print!(
            "\x1b[2J\x1b[H{body}\n{notice}\nUpdated {}s\nCommands: renew ID FENCE [MINUTES] | release ID FENCE [NOTE] | answer ID TEXT | cancel ID REASON | quit",
            crate::log::now_ms() / 1_000
        );
        std::io::stdout()
            .flush()
            .map_err(|_| Usage("cannot write the watch display".into()))?;
        seen += 1;
        if iterations != 0 && seen >= iterations {
            return Ok(EXIT_OK);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(interval);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining) {
                Ok(line) => {
                    let line = line.trim();
                    if line.eq_ignore_ascii_case("quit") || line.eq_ignore_ascii_case("q") {
                        return Ok(EXIT_OK);
                    }
                    if line.is_empty() {
                        continue;
                    }
                    notice = watch_command(&client, line);
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(remaining);
                    break;
                }
            }
        }
    }
}

fn mcp(flags: &Flags) -> Result<i32, Usage> {
    let client = server(flags)?;
    let mut session = McpServer::new(client, flags.get("--tool").cloned());
    let stdin = std::io::stdin().lock();
    let stdout = std::io::stdout().lock();
    Ok(match session.run(stdin, stdout) {
        Ok(()) => EXIT_OK,
        Err(error) => refused(&json!({ "error": error })),
    })
}

fn check_config(flags: &Flags) -> i32 {
    let path = PathBuf::from(flags.get("--config").cloned().unwrap_or_default());
    match FleetConfig::load(&path) {
        Ok(config) => {
            println!(
                "{}",
                json!({
                    "valid": true,
                    "schema": config.schema,
                    "machine": config.machine,
                    "listen": config.listen,
                    "dispatch": config.dispatch.enabled,
                    "swarm": config.active_swarm().is_some(),
                })
            );
            EXIT_OK
        }
        Err(error) => refused(&json!({ "valid": false, "error": error })),
    }
}

fn run(flags: &Flags) -> i32 {
    let path = PathBuf::from(flags.get("--config").cloned().unwrap_or_default());
    let console_url = flags.get("--console-url").map(String::as_str);
    match serve_with_console_url(&path, console_url) {
        Ok(()) => EXIT_OK,
        Err(error) => {
            log::warn("service.failed", json!({ "error": error }));
            refused(&json!({ "error": error }))
        }
    }
}

/// Start the API listeners and the dispatch thread, then wait on them.
///
/// # Errors
/// Returns a start-up diagnostic.
pub fn serve(path: &std::path::Path) -> Result<(), String> {
    serve_with_console_url(path, None)
}

fn serve_with_console_url(
    path: &std::path::Path,
    cli_console_url: Option<&str>,
) -> Result<(), String> {
    let mut config = FleetConfig::load(path)?;
    if let Some(raw) = cli_console_url {
        config.console_url = Some(validate_console_url(raw)?);
    } else if config.console_url.is_none() {
        match std::env::var(CONSOLE_URL_ENVIRONMENT) {
            Ok(raw) => config.console_url = Some(validate_console_url(&raw)?),
            Err(std::env::VarError::NotPresent) => {}
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(format!("{CONSOLE_URL_ENVIRONMENT} is not valid UTF-8"));
            }
        }
    }
    let addresses = config.listen_addresses()?;
    let service = Arc::new(Service::new(config)?);
    let listeners = server::bind(&addresses)?;
    // The dispatcher owns a non-Send store connection, so it is built on its
    // own thread and reports start-up through a channel.
    let (ready, started) = std::sync::mpsc::channel::<Result<(), String>>();
    let dispatch_service = Arc::clone(&service);
    let dispatch = std::thread::Builder::new()
        .name("kiln-fleet-dispatch".into())
        .spawn(move || match Dispatcher::new(dispatch_service) {
            Ok(dispatcher) => {
                let _ = ready.send(Ok(()));
                dispatcher.run();
            }
            Err(error) => {
                let _ = ready.send(Err(error));
            }
        })
        .map_err(|error| format!("cannot start the dispatch thread: {error}"))?;
    started
        .recv()
        .map_err(|_| "the dispatch thread stopped during start-up".to_owned())??;
    if service.config.herdr_core.is_some() {
        let observer = Arc::clone(&service);
        if std::thread::Builder::new()
            .name("kiln-herdr-core-observer".into())
            .spawn(move || observer.observe_core())
            .is_err()
        {
            log::warn("core.observer_thread_unavailable", json!({}));
        }
    }
    let api = server::spawn(listeners, &service)?;
    log::info(
        "service.started",
        json!({ "machine": service.config.machine, "listen": service.config.listen, "dispatch": service.config.dispatch.enabled }),
    );
    let _ = dispatch.join();
    for handle in api {
        let _ = handle.join();
    }
    Err("the service threads stopped".to_owned())
}
