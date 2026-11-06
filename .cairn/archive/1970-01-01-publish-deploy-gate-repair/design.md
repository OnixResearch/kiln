## Context

At preflight, local main and both tracked remotes named `d80edf75`; the committed candidate descends from it. The desktop user unit resolves its executable through the user's Nix profile and supplies eight configuration files. Its prior package was `/nix/store/lg8vwxjqb04pxb1wi1xvr2j4bk0d8c36-kiln-web-service-0.1.0`.

## Decisions

Freeze source at `946e2d29c6d3173b8f98a8a6d6e184d85db61af6`. Refresh remote observations, require fast-forward ancestry, use jj pushes through existing remotes, preserve the single-delegate threshold-one public main identity and observe signed refs before/after. Reconstruct the declared normalized USTAR and sorted path manifest for candidate and independent acquisitions.

Build from that immutable source, not the evidence working copy. Check the candidate against the unchanged installed argument set. Retain private configuration/profile metadata and consistent SQLite backups; do not put secrets or database contents in repository evidence. Capture the prior package for targeted package rollback. Replace only the web-service profile element and issue a restart only for its user service; verify the running executable, HTTPS health, anonymous refusal, authorized read access and actual browser rendering. Preserve Lattice, proxy/auth configuration, system Aspen units and isolation/approval policies. The existing proxy/auth units require the web service and restarted during activation; their healthy new PIDs are recorded rather than claimed unchanged. Lattice stayed on its original PID. On activation failure, restore the prior package and restart without rewinding live SQLite or collaboration refs.

## No-spec classification

This operation changes remote refs and operator runtime state under explicit authority, but no accepted product behavior or requirement meaning. Evidence and lifecycle prose are later descendants of the frozen source. The classification review is an assistant plan assessment, not independent human approval or release authority.

## Risks / Trade-offs

Publication is externally visible; stop rather than force a moved/diverged ref. Service restart briefly interrupts requests. Existing state must survive; no automatic database rewind is permitted. Unavailable signer, failed preflight, incompatible host policy or failed health blocks activation rather than authorizing bypass. Release readiness and operator-only hardware qualification remain separate.
