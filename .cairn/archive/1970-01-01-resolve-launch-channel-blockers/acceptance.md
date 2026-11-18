# Change-local acceptance

a[resolve-launch-channel-blockers.settings] Private vulnerability reporting, secret scanning and push protection read back as enabled on `OnixResearch/kiln`, its description and topics read back as set, and a ruleset that blocks deletion and non-fast-forward pushes reads back as active on its `main`, with the state before each change retained. No other setting of that repository changes.

a[resolve-launch-channel-blockers.profile] The organization profile's project table reads back with exactly one added row for Kiln, in one commit whose parent is the head observed before the change, and the anonymous organization page links `OnixResearch/kiln`.

a[resolve-launch-channel-blockers.withdrawal] `OnixResearch/kiln-public` reads back as private, an anonymous request for it fails, `OnixResearch/kiln` stays anonymously reachable, nothing is deleted, and `docs/hosting-and-recovery.md` carries a dated withdrawal record that leaves the earlier projection record unchanged.

a[resolve-launch-channel-blockers.kit] The launch kit lists the resolved blockers as resolved with their dates, keeps every remaining owner action open, shows no command that was not run as run, quotes test and coverage figures observed on the current tree, and states which Nix gates ran and which did not. Every external link in the public documents resolves, except the dated record of the withdrawn repository.

a[resolve-launch-channel-blockers.verification] Read-backs, anonymous reachability, rerun commands, link checks and lifecycle gates are exercised, and the retained evidence names what was not exercised.
