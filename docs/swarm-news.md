# Swarm claim mirror

Optional, default-off adapter for Britton's private swarm newsgroups
(`bots.claims`, `bots.announce`, `bots.general`), served by renews over iroh. It
lets a Kiln implementation attempt announce `CLAIM <repository> <scope>` and
`RELEASE` the same scope, and lets admission defer when another agent already
holds an open claim.

Articles are untrusted data. The swarm coordinates work; it never locks,
authorizes, cancels or approves anything in Kiln. Kiln's durable claims, leases
and fences ([implementation work](implementation-work.md)) remain authoritative.

## Trust and credentials

`kiln-adapter-swarm` runs a pinned `iroh-news` executable as a bounded child
process. Kiln passes no password, endpoint key or ticket. `iroh-news` resolves its
own credentials through SecretSpec (project `swarm-news` on deployed machines) or
its state directory. The child environment is cleared and rebuilt only from the
explicit `environment` map; argv is fixed; stdout and stderr are bounded; the
process group is terminated on deadline.

Linking `iroh-news` is not used: it targets iroh 1.4 and Kiln pins 0.95, and the
client owns its own identity and provider.

## Configuration

`SwarmConfig` (JSON, `--config-json`) fields:

| Field | Meaning |
|---|---|
| `enabled` | explicit enable flag; false makes no connection |
| `executable` | absolute path to the pinned `iroh-news` |
| `state_dir` | absolute client state directory (key, cursors) |
| `config_file` | optional generated `iroh-news` config |
| `server` | optional ticket, endpoint id or DNS name |
| `group` | claim group, default `bots.claims` |
| `from` | our `From` header, used to recognize our own claims |
| `timeout_ms` | per-command deadline, default 30 s |
| `read_limit` | maximum articles per read, default 50 |
| `max_output_bytes` | retained stream bound, default 1 MiB |
| `no_relays` | dial ticket addresses only (typical for the tailnet) |
| `environment` | explicit child environment; ambient variables are cleared |

Password- and key-named environment entries are refused.

## Implementer integration

When the operator passes `--swarm-config-json PATH` (and the profile has
`"enabled": true`), the implementer `claim` command reads recent claims, defers
overlapping work with a structured stderr cause, and posts the `CLAIM` through
the mirror journal. `cancel` posts the matching `RELEASE`. An unavailable swarm
never fails the command. `--swarm-host` sets the signature host label.

## Operator CLI

```console
kiln-swarm read --config-json /etc/kiln/swarm.json --since 120 --limit 20
printf '%s\n' "$body" | kiln-swarm post --config-json /etc/kiln/swarm.json \
  --subject 'CLAIM OnixResearch/kiln crates/kiln-core'
printf '%s\n' "$body" | kiln-swarm post --config-json /etc/kiln/swarm.json \
  --subject 'RELEASE OnixResearch/kiln crates/kiln-core' --references '<claim@aspen1>'
```

`read` prints the article array or exits non-zero when the group is unavailable.
`post` prints `{"outcome": "...", "detail": "..."}` and exits non-zero only for a
local or server refusal.

## Outcomes and overlap

- `accepted`: the server accepted the article.
- `rejected`: the server refused it, or Kiln refused the subject/body locally.
- `unknown`: the deadline expired or the response was lost; the article may exist.
  Do not blindly repost.
- `unavailable`: the client could not start; nothing was sent.

A prospective claim defers only on a recent `CLAIM` from another author on the
same repository and scope with no `RELEASE` referencing it and younger than 24
hours. Our own claims, released claims, stale claims and other scopes do not
defer.

Bodies are rendered from named facts and refused when they contain secret
references, bearer headers, token-shaped values or control characters.

## Non-claims

The mirror proves no effect completion, worker termination, resource release or
release eligibility. Swarm availability is not required for admission, and a
swarm failure does not fail an attempt.
