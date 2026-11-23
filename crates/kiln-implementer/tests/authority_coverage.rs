//! Source-scanning coverage for `kiln.implementation-authority-coverage.v2`.
//!
//! Every `Implementer` public function, `kiln-implementer` CLI subcommand,
//! fleet API route and web campaign control route must have a reviewed row in
//! `ENTRY_POINTS`. A new entry point without a row, a row for an entry point
//! that no longer exists, a `Protected(op)` row whose code never reaches the
//! `op` authority check, and a non-authority row whose body checks authority
//! all fail this test.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kiln_core::implementation::authority::{
    ENTRY_POINTS, EntryClass, EntryPoint, IMPLEMENTER_CLI, ProtectedOperation, entry_point_roles,
    validate_entry_points,
};
use kiln_core::implementation::grant::GrantRole;

const IMPLEMENTER_SRC: &str = "crates/kiln-implementer/src";
const CLI: &str = "crates/kiln-adapter-github/src/bin/implementer.rs";
const FLEET_API: &str = "crates/kiln-fleet/src/api.rs";
const CONTROLS: &str = "crates/kiln-web-service/src/campaign_controls.rs";
const MAX_CALL_DEPTH: usize = 8;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| unreachable!("{relative}: {error}"))
}

fn blank(out: &mut [u8], from: usize, to: usize) {
    for byte in out.iter_mut().take(to).skip(from) {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
}

/// Blank comments and string/char literal contents, keeping byte offsets, so
/// brace matching and token searches see code only.
fn code_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut index = 0;
    while index < bytes.len() {
        let rest = bytes.get(index..).unwrap_or_default();
        if rest.starts_with(b"//") {
            let end = rest
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            blank(&mut out, index, end);
            index = end;
        } else if rest.starts_with(b"/*") {
            let mut depth = 0usize;
            let mut cursor = index;
            while cursor < bytes.len() {
                let tail = bytes.get(cursor..).unwrap_or_default();
                if tail.starts_with(b"/*") {
                    depth += 1;
                    cursor += 2;
                } else if tail.starts_with(b"*/") {
                    depth -= 1;
                    cursor += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    cursor += 1;
                }
            }
            blank(&mut out, index, cursor);
            index = cursor;
        } else if (rest.starts_with(b"r#") || rest.starts_with(b"r\""))
            && raw_prefix_allowed(bytes, index)
        {
            let hashes = rest
                .iter()
                .skip(1)
                .take_while(|byte| **byte == b'#')
                .count();
            if rest.get(1 + hashes) != Some(&b'"') {
                index += 1;
                continue;
            }
            let mut closing = vec![b'"'];
            closing.extend(std::iter::repeat_n(b'#', hashes));
            let start = index + 2 + hashes;
            let end = bytes
                .get(start..)
                .and_then(|tail| tail.windows(closing.len()).position(|w| w == closing))
                .map_or(bytes.len(), |offset| start + offset + closing.len());
            blank(&mut out, start, end.saturating_sub(closing.len()));
            index = end;
        } else if rest.first() == Some(&b'"') {
            let mut cursor = index + 1;
            while cursor < bytes.len() && bytes.get(cursor) != Some(&b'"') {
                cursor += if bytes.get(cursor) == Some(&b'\\') {
                    2
                } else {
                    1
                };
            }
            blank(&mut out, index + 1, cursor);
            index = cursor + 1;
        } else if rest.first() == Some(&b'\'') {
            // A char literal is `'x'` or `'\…'`; anything else is a lifetime.
            let end = if rest.get(1) == Some(&b'\\') {
                rest.iter()
                    .skip(2)
                    .position(|byte| *byte == b'\'')
                    .map(|offset| index + 2 + offset + 1)
            } else {
                let width = std::str::from_utf8(rest.get(1..).unwrap_or_default())
                    .ok()
                    .and_then(|tail| tail.chars().next())
                    .map_or(1, char::len_utf8);
                (rest.get(1 + width) == Some(&b'\'')).then_some(index + 2 + width)
            };
            match end {
                Some(end) => {
                    blank(&mut out, index + 1, end - 1);
                    index = end;
                }
                None => index += 1,
            }
        } else {
            index += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|error| unreachable!("{error}"))
}

/// A raw string starts at a word boundary, or after the `b` of `br`.
fn raw_prefix_allowed(bytes: &[u8], index: usize) -> bool {
    let before = |back: usize| {
        index
            .checked_sub(back)
            .and_then(|at| bytes.get(at))
            .copied()
    };
    match before(1) {
        None => true,
        Some(b'b') => !before(2).is_some_and(is_word),
        Some(byte) => !is_word(byte),
    }
}

/// Index of the brace that closes the one opened at `open`.
fn matching_brace(code: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (offset, byte) in code.bytes().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return offset;
                }
            }
            _ => {}
        }
    }
    unreachable!("unbalanced braces")
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Every `fn name` with a body in `code`, by name.
fn functions(code: &str) -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let bytes = code.as_bytes();
    let mut search = 0;
    while let Some(offset) = code.get(search..).and_then(|tail| tail.find("fn ")) {
        let at = search + offset;
        search = at + 3;
        if at > 0 && bytes.get(at - 1).copied().is_some_and(is_word) {
            continue;
        }
        let name: String = code
            .get(at + 3..)
            .unwrap_or_default()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let tail = code.get(at..).unwrap_or_default();
        let (Some(open), semicolon) = (tail.find('{'), tail.find(';')) else {
            continue;
        };
        if semicolon.is_some_and(|semicolon| semicolon < open) {
            continue;
        }
        let close = matching_brace(code, at + open);
        found
            .entry(name)
            .or_default()
            .push(code.get(at + open..=close).unwrap_or_default().to_owned());
    }
    found
}

/// Names of calls in `body` matching `prefix` + `name(`.
fn calls(body: &str, prefix: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let bytes = body.as_bytes();
    let mut search = 0;
    while let Some(offset) = body.get(search..).and_then(|tail| tail.find('(')) {
        let paren = search + offset;
        search = paren + 1;
        let start = bytes
            .get(..paren)
            .unwrap_or_default()
            .iter()
            .rposition(|byte| !is_word(*byte))
            .map_or(0, |position| position + 1);
        let name = body.get(start..paren).unwrap_or_default();
        if name.is_empty() || name.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            continue;
        }
        // Method chains may break the line between the receiver and the dot.
        let before = body.get(..start).unwrap_or_default().trim_end();
        let receiver = before.strip_suffix('.').map(str::trim_end);
        let matched = match prefix {
            "self." => {
                before.ends_with("Self::")
                    || receiver.is_some_and(|receiver| {
                        receiver.strip_suffix("self").is_some_and(|rest| {
                            !rest.as_bytes().last().copied().is_some_and(is_word)
                        })
                    })
            }
            "." => receiver.is_some(),
            _ => receiver.is_none(),
        };
        if matched {
            names.insert(name.to_owned());
        }
    }
    names
}

fn mentions(body: &str, operation: ProtectedOperation) -> bool {
    let token = format!("ProtectedOperation::{operation:?}");
    body.match_indices(&token).any(|(at, _)| {
        !body
            .as_bytes()
            .get(at + token.len())
            .copied()
            .is_some_and(is_word)
    })
}

fn direct_operations(body: &str) -> BTreeSet<ProtectedOperation> {
    ProtectedOperation::ALL
        .into_iter()
        .filter(|operation| mentions(body, *operation))
        .collect()
}

/// The implementer crate: every function body by name, and each public
/// `Implementer` method with its defining file and own body.
struct Implementer {
    functions: BTreeMap<String, Vec<String>>,
    public: BTreeMap<String, (String, String)>,
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| unreachable!("{error}"))
        .map(|entry| entry.unwrap_or_else(|error| unreachable!("{error}")).path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn implementer() -> Implementer {
    let mut files = Vec::new();
    rust_files(&root().join(IMPLEMENTER_SRC), &mut files);
    let mut every: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut public = BTreeMap::new();
    for file in files {
        let relative = file
            .strip_prefix(root())
            .unwrap_or_else(|error| unreachable!("{error}"))
            .to_string_lossy()
            .into_owned();
        let code = code_only(
            &std::fs::read_to_string(&file).unwrap_or_else(|error| unreachable!("{error}")),
        );
        for (name, bodies) in functions(&code) {
            every.entry(name).or_default().extend(bodies);
        }
        let mut search = 0;
        while let Some(offset) = code
            .get(search..)
            .and_then(|tail| tail.find("\nimpl Implementer {"))
        {
            let open = search + offset + "\nimpl Implementer ".len();
            let close = matching_brace(&code, open);
            let block = code.get(open..close).unwrap_or_default();
            let methods = functions(block);
            for line in block.lines() {
                if let Some(signature) = line.strip_prefix("    pub fn ") {
                    let name: String = signature
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    let body = methods
                        .get(&name)
                        .and_then(|bodies| bodies.first())
                        .cloned()
                        .unwrap_or_else(|| unreachable!("Implementer::{name} body"));
                    let previous = public.insert(name.clone(), (relative.clone(), body));
                    assert!(previous.is_none(), "Implementer::{name} defined twice");
                }
            }
            search = close;
        }
    }
    Implementer {
        functions: every,
        public,
    }
}

impl Implementer {
    /// Whether `body` reaches an `operation` check through `self` methods
    /// and crate functions.
    fn reaches(
        &self,
        body: &str,
        operation: ProtectedOperation,
        seen: &mut BTreeSet<String>,
    ) -> bool {
        mentions(body, operation)
            || calls(body, "self.")
                .into_iter()
                .chain(calls(body, ""))
                .any(|callee| {
                    if !seen.insert(callee.clone()) {
                        return false;
                    }
                    let own = self.public.get(&callee).map(|(_, body)| body.as_str());
                    match own {
                        Some(body) => self.reaches(body, operation, seen),
                        None => self.functions.get(&callee).is_some_and(|bodies| {
                            bodies
                                .iter()
                                .any(|body| self.reaches(body, operation, seen))
                        }),
                    }
                })
    }

    fn protected_rows(&self, name: &str) -> BTreeSet<ProtectedOperation> {
        ENTRY_POINTS
            .iter()
            .filter(|entry| {
                self.public
                    .get(name)
                    .is_some_and(|(path, _)| path == entry.path)
            })
            .filter(|entry| entry.item == name)
            .filter_map(|entry| match entry.class {
                EntryClass::Protected(operation) => Some(operation),
                _ => None,
            })
            .collect()
    }
}

/// Whether a shell handler reaches an `Implementer` method with a
/// `Protected(operation)` row, through its own functions.
fn shell_reaches(
    shell: &BTreeMap<String, Vec<String>>,
    implementer: &Implementer,
    handler: &str,
    operation: ProtectedOperation,
    depth: usize,
    seen: &mut BTreeSet<String>,
) -> bool {
    if depth > MAX_CALL_DEPTH || !seen.insert(handler.to_owned()) {
        return false;
    }
    let Some(bodies) = shell.get(handler) else {
        return false;
    };
    bodies.iter().any(|body| {
        calls(body, ".").iter().any(|method| {
            implementer.public.contains_key(method)
                && implementer.protected_rows(method).contains(&operation)
        }) || calls(body, "")
            .into_iter()
            .chain(calls(body, "self."))
            .any(|callee| shell_reaches(shell, implementer, &callee, operation, depth + 1, seen))
    })
}

fn rows(path: &str) -> Vec<&'static EntryPoint> {
    ENTRY_POINTS
        .iter()
        .filter(|entry| entry.path == path)
        .collect()
}

fn assert_shell_rows(
    path: &str,
    entry_points: &BTreeMap<String, Option<String>>,
    shell: &BTreeMap<String, Vec<String>>,
    implementer: &Implementer,
) {
    for item in entry_points.keys() {
        assert!(
            rows(path).iter().any(|entry| entry.item == item),
            "{path}: entry point `{item}` has no ENTRY_POINTS row"
        );
    }
    for entry in rows(path) {
        let Some(handler) = entry_points.get(entry.item) else {
            unreachable!("{path}: row `{}` names no entry point", entry.item);
        };
        if let EntryClass::Protected(operation) = entry.class {
            let handler = handler
                .as_deref()
                .unwrap_or_else(|| unreachable!("{path}: `{}` has no handler", entry.item));
            assert!(
                shell_reaches(
                    shell,
                    implementer,
                    handler,
                    operation,
                    0,
                    &mut BTreeSet::new()
                ),
                "{path}: `{}` is Protected({operation:?}) but never reaches it",
                entry.item
            );
        }
    }
}

#[test]
// r[verify kiln.implementation.authority.coverage.entry_points]
// r[verify kiln.implementation.authority.coverage.entry_points.scenario.unlisted]
fn every_implementer_public_function_has_a_reviewed_row_that_matches_its_code() {
    validate_entry_points(ENTRY_POINTS).unwrap_or_else(|error| unreachable!("{error:?}"));
    let implementer = implementer();
    assert!(
        implementer.public.len() > 90,
        "scan found {}",
        implementer.public.len()
    );
    for (name, (path, _)) in &implementer.public {
        assert!(
            ENTRY_POINTS
                .iter()
                .any(|entry| entry.path == path && entry.item == name),
            "Implementer::{name} ({path}) has no ENTRY_POINTS row"
        );
    }
    for entry in ENTRY_POINTS
        .iter()
        .filter(|entry| entry.path.starts_with(IMPLEMENTER_SRC))
    {
        assert_eq!(
            implementer
                .public
                .get(entry.item)
                .map(|(path, _)| path.as_str()),
            Some(entry.path),
            "row `{}` names no public Implementer method in its file",
            entry.item
        );
    }
    for (name, (path, body)) in &implementer.public {
        let classes: Vec<EntryClass> = ENTRY_POINTS
            .iter()
            .filter(|entry| entry.path == path && entry.item == name)
            .map(|entry| entry.class)
            .collect();
        let named: BTreeSet<ProtectedOperation> = classes
            .iter()
            .filter_map(|class| class.operation())
            .collect();
        for class in &classes {
            if let EntryClass::Protected(operation) = class {
                assert!(
                    implementer.reaches(body, *operation, &mut BTreeSet::new()),
                    "Implementer::{name} is Protected({operation:?}) but never checks it"
                );
            }
        }
        // A body that checks an operation directly names it in its rows, and
        // a purely non-authority entry point checks none.
        for operation in direct_operations(body) {
            assert!(
                named.contains(&operation),
                "Implementer::{name} checks {operation:?} without a row naming it"
            );
        }
    }
}

/// Every CLI subcommand and the shell function its dispatch arm calls.
fn cli_subcommands(source: &str) -> BTreeMap<String, Option<String>> {
    let start = source
        .find("match command.as_str() {")
        .unwrap_or_else(|| unreachable!("CLI dispatch"));
    let block = source
        .get(start..matching_brace(&code_only(source), start + "match command.as_str() ".len()))
        .unwrap_or_default();
    let mut subcommands = BTreeMap::new();
    let lines: Vec<&str> = block.lines().skip(1).map(str::trim).collect();
    for (index, line) in lines.iter().enumerate() {
        let Some((pattern, handler)) = line.split_once("=>") else {
            continue;
        };
        if !pattern.starts_with('"') {
            continue;
        }
        // A block arm names its handler on the next line.
        let handler = match handler.trim() {
            "{" => lines.get(index + 1).copied().unwrap_or_default(),
            inline => inline,
        };
        let name: String = handler
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        for alternative in pattern.split('|') {
            let subcommand = alternative.trim().trim_matches('"');
            if !subcommand.is_empty() {
                subcommands.insert(
                    subcommand.to_owned(),
                    (!name.is_empty()).then(|| name.clone()),
                );
            }
        }
    }
    subcommands
}

/// The operations an `Implementer` method's reviewed rows name, and those of
/// every public method it calls on `self`. The rows are the reviewed bound:
/// the first coverage test proves each body checks only what its rows name.
///
/// A multiplexed method (one with an `OperatorConfig` row beside protected
/// rows, such as `admission_apply`) is not followed from another method: the
/// caller issues only some of its commands, and its own rows name the
/// operations those commands reach.
fn method_operations(
    implementer: &Implementer,
    method: &str,
    seen: &mut BTreeSet<String>,
    found: &mut BTreeSet<ProtectedOperation>,
) {
    if !seen.insert(method.to_owned()) {
        return;
    }
    let Some((path, body)) = implementer.public.get(method) else {
        return;
    };
    found.extend(
        ENTRY_POINTS
            .iter()
            .filter(|entry| entry.path == path && entry.item == method)
            .filter_map(|entry| entry.class.operation()),
    );
    for callee in calls(body, "self.") {
        let rows: Vec<EntryClass> = implementer
            .public
            .get(&callee)
            .map(|(path, _)| {
                ENTRY_POINTS
                    .iter()
                    .filter(|entry| entry.path == path && entry.item == callee)
                    .map(|entry| entry.class)
                    .collect()
            })
            .unwrap_or_default();
        let multiplexed = rows.contains(&EntryClass::OperatorConfig)
            && rows.iter().any(|class| class.operation().is_some());
        if !multiplexed {
            method_operations(implementer, &callee, seen, found);
        }
    }
}

/// Every operation a shell handler reaches through the `Implementer` methods
/// it calls, directly or through its own functions.
fn shell_operations(
    shell: &BTreeMap<String, Vec<String>>,
    implementer: &Implementer,
    handler: &str,
    depth: usize,
    seen: &mut BTreeSet<String>,
    found: &mut BTreeSet<ProtectedOperation>,
) {
    if depth > MAX_CALL_DEPTH || !seen.insert(handler.to_owned()) {
        return;
    }
    let Some(bodies) = shell.get(handler) else {
        return;
    };
    for body in bodies {
        for method in calls(body, ".") {
            method_operations(implementer, &method, &mut BTreeSet::new(), found);
        }
        for callee in calls(body, "").into_iter().chain(calls(body, "self.")) {
            shell_operations(shell, implementer, &callee, depth + 1, seen, found);
        }
    }
}

#[test]
// r[verify kiln.implementation.authority.coverage.entry_points]
fn every_cli_subcommand_has_a_reviewed_row_that_matches_its_handler() {
    let source = read(CLI);
    let subcommands = cli_subcommands(&source);
    assert!(subcommands.len() > 40, "scan found {}", subcommands.len());
    assert_shell_rows(
        CLI,
        &subcommands,
        &functions(&code_only(&source)),
        &implementer(),
    );
}

#[test]
// r[verify kiln.implementation.authority.selection.scenario.scoped]
fn every_cli_subcommand_loads_the_holder_role_of_every_check_it_reaches() {
    assert_eq!(CLI, IMPLEMENTER_CLI);
    let source = read(CLI);
    let shell = functions(&code_only(&source));
    let implementer = implementer();
    let subcommands = cli_subcommands(&source);
    let mut missing = Vec::new();
    for (subcommand, handler) in &subcommands {
        let Some(handler) = handler else {
            continue;
        };
        let mut reached = BTreeSet::new();
        shell_operations(
            &shell,
            &implementer,
            handler,
            0,
            &mut BTreeSet::new(),
            &mut reached,
        );
        // Arms that share one handler (the `host-owner-*` commands) dispatch
        // again inside it, which this scan cannot follow: compare against the
        // roles of the whole group. A subcommand that reached another arm's
        // check at run time would be refused as holder-not-loaded, not
        // widened.
        let loaded: BTreeSet<GrantRole> = subcommands
            .iter()
            .filter(|(_, other)| other.as_deref() == Some(handler.as_str()))
            .flat_map(|(other, _)| entry_point_roles(ENTRY_POINTS, IMPLEMENTER_CLI, other))
            .collect();
        missing.extend(
            reached
                .into_iter()
                .filter(|operation| !loaded.contains(&GrantRole::of(*operation)))
                .map(|operation| format!("`{subcommand}` reaches {operation:?}")),
        );
    }
    assert!(
        missing.is_empty(),
        "subcommands reach checks whose holder role they do not load: {missing:?}"
    );
}

#[test]
// r[verify kiln.implementation.authority.coverage.entry_points]
fn every_fleet_route_has_a_reviewed_row_that_matches_its_handler() {
    let source = read(FLEET_API);
    let mut routes = BTreeMap::new();
    let lines: Vec<&str> = source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("(\"") else {
            continue;
        };
        let Some((method, rest)) = rest.split_once("\", [") else {
            continue;
        };
        let Some((segments, rest)) = rest.split_once("])") else {
            continue;
        };
        if !rest.trim_start().starts_with("=>") {
            continue;
        }
        let path: Vec<String> = segments
            .split(',')
            .map(str::trim)
            .map(|segment| match segment.strip_prefix('"') {
                Some(literal) => literal.trim_end_matches('"').to_owned(),
                None => format!("{{{segment}}}"),
            })
            .collect();
        let arm = format!(
            "{rest} {}",
            lines.get(index + 1).copied().unwrap_or_default()
        );
        let handler = arm.split_once("self.").map(|(_, call)| {
            call.chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
        });
        routes.insert(format!("{method} /v1/{}", path.join("/")), handler);
    }
    assert!(routes.len() >= 13, "scan found {}", routes.len());
    assert_shell_rows(
        FLEET_API,
        &routes,
        &functions(&code_only(&source)),
        &implementer(),
    );
}

#[test]
// r[verify kiln.implementation.authority.coverage.entry_points]
fn every_campaign_control_route_has_a_reviewed_row() {
    let source = read(CONTROLS);
    let routes: BTreeSet<String> = source
        .lines()
        .filter_map(|line| line.strip_prefix("pub const "))
        .filter(|line| line.contains("_ROUTE: &str = \""))
        .filter_map(|line| line.split('"').nth(1).map(str::to_owned))
        .collect();
    assert_eq!(routes.len(), 3, "{routes:?}");
    let reviewed = rows(CONTROLS);
    for route in &routes {
        let method = if route.ends_with("/commands") {
            "POST"
        } else {
            "GET"
        };
        assert!(
            reviewed
                .iter()
                .any(|entry| entry.item == format!("{method} {route}")),
            "campaign control route `{method} {route}` has no ENTRY_POINTS row"
        );
    }
    for entry in reviewed {
        assert!(
            entry
                .item
                .split_once(' ')
                .is_some_and(|(_, route)| routes.contains(route)),
            "row `{}` names no campaign control route",
            entry.item
        );
        assert!(
            entry.class.operation().is_none(),
            "campaign controls apply typed intents; the drive admits their effects"
        );
    }
}

#[test]
fn the_scanner_sees_code_not_comments_or_strings() {
    let code = code_only(
        "fn a() { // ProtectedOperation::Dispatch {\n let s = \"}{ ProtectedOperation::Dispatch\"; let c = '{'; let r = r#\"}\"#; }\nfn b<'x>(v: &'x str) { self.a(); }",
    );
    let found = functions(&code);
    assert!(
        direct_operations(
            found
                .get("a")
                .and_then(|b| b.first())
                .map_or("", String::as_str)
        )
        .is_empty()
    );
    assert_eq!(
        found
            .get("b")
            .and_then(|bodies| bodies.first())
            .map(|body| calls(body, "self.")),
        Some(BTreeSet::from(["a".to_owned()]))
    );
}
