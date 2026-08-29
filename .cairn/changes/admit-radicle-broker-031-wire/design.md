# Design: Bounded Broker 0.31 Projection

## Ownership

Radicle CI broker owns its request envelope and descriptive repository and author metadata. Kiln owns trigger identity, admission, lifecycle, and terminal meaning. The Defelo adapter is the translation boundary. Aspen and Lattice remain downstream mechanisms and receive no broker-native types.

## Producer Contract

Broker `0.31` serializes `radicle_ci_adapter::Request::Trigger`. Every request has:

- `request = "trigger"`;
- `version = 1`;
- `event_type`;
- a repository object; and
- push or patch fields.

The broker's published `trigger.json` includes repository description, visibility, default branch, delegates, pusher, before, and after. Its own commit selection uses the first push commit or first patch commit.

## Projection

Kiln requires and validates request kind and version. It decodes the existing bounded projection:

- `event_type`;
- `repository.id`;
- `commits` for a push;
- `patch.commits` for a patch; and
- optional `branch`.

Other producer-owned fields remain non-authoritative metadata. Serde may ignore them because Kiln neither interprets nor preserves them. This is narrower than importing broker types or treating every producer field as CI policy.

The mapped actor remains the explicit `broker` boundary identity. Push `before` remains absent because broker `0.31` can report the tip as both `before` and `after` when it projects a canonical reference update. Kiln's exact trigger identity still binds repository, selected commit, branch, event kind, and broker actor.

## Failure Boundary

Missing or changed request kind, wrong version, unknown event kind, malformed JSON, empty input, overbound input, or no selected commit fails before host transport. Extra metadata never supplies a missing authoritative field.

## Tests

Positive tests use the producer's checked-in `trigger.json` shape. Negative tests cover wrong or missing envelope identity, unknown event, missing commit, malformed JSON, and bounds. Existing response and terminal-classification tests remain required.
