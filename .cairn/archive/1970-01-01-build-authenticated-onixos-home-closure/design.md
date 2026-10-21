# Design: authenticated OnixOS home closure

## Decisions

The existing real inventory and service remain the source of the home application. The Nix projection exposes launcher, activation and home-file derivations under a separate preparation selector; the authenticated selector still selects one `home-activation` task. A BLAKE3 build identity is inserted into the inventory's actual `git_user` setting, rather than an unrelated stamp derivation. This changes the generated file and its transitive consumers without executing any activation command.

The negative selector wraps the real service implementation with one additional file whose target is beneath the existing `.gitconfig` file. The consumer's normal home-file builder must fail when it tries to create that child. It is neither a transport mock nor an appended failing launcher command.

## Functional core and imperative shell

A pure Python helper admits exactly one launcher, activation and home-file record, checks their dependency chain, deterministically computes external dependencies and rejects consumer members in a supplied external dependency closure. It performs no filesystem, environment, process, clock or network operations. Positive and negative regressions exercise shared dependencies, malformed or incomplete records and a consumer reachable through an allegedly external dependency.

The existing bounded runner discovers these records using the explicit Onix evaluator, observes the external derivation closure, obtains the pure preparation decision, realises only admitted external dependencies and records their output closure. Immediately before dispatch it requires all three consumer outputs to be absent. After success it checks the generated Git configuration against the bound identity, verifies the launcher closure contains activation and home files and retains their native identities. After the negative run it requires the exact conflicting-target diagnostic and no realised consumer outputs. Evaluation failure remains an actual evaluator failure without a build plan.

Native Nix path-info can return exit zero with a null value for an absent output; admission uses the exact path's metadata, not the process exit alone. A failed builder can leave a physical but unregistered partial directory. The runner preserves both facts, rejects any pre-existing physical output before dispatch and never promotes an unregistered failed directory to a realised product.

## Existing contracts

No Rust provider, canonical CI transition, signed-request, workflow, restart, root-retention or UCAN contract changes. Existing exact executable cohorts and native algorithms remain explicit. The new source projection, helper and runner bytes are measured in the retained cohort. Runtime preparation may acquire external inputs; the authenticated provider keeps its existing no-substitution and no-remote-builder policy.

## Alternatives rejected

- Removing preparation entirely would require rebuilding or acquiring the full external toolchain under the bounded local CI policy and would obscure the consumer boundary.
- Recognising generated outputs by path-name prefixes would silently misclassify a changed consumer. Explicit evaluated derivation identities and transitive exclusion provide the boundary instead.
- Reusing cached consumer outputs or renaming only the launcher would not prove fresh home-file or activation construction.
- Appending `exit` to the launcher would not exercise a failing consumer home-file build.

## Verification and risks

Run pure preparation-policy regressions and their Nix check, then the actual success, home-file failure and evaluation failure cases from an immutable source snapshot. Each case retains signed-request denials, pending replay across host restart, one original provider invocation, canonical outcome, root handoff/release, UCAN view identities and credential/process retirement. Record the external preparation closure and consumer absence immediately before authentication; success must observe all expected outputs only after the actual worker completes.

The qualification remains scoped to the selected x86_64-linux consumer and executable cohort. Reject already realised consumer outputs rather than deleting shared store paths. The target collision depends on the reviewed consumer's deterministic file order; retain its exact native diagnostic. Do not expand this change into activation, forge delivery, production state, publication or release approval.
