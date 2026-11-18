# Source projection delta

## ADDED Requirements

### Requirement: Planned build inputs for projected views

r[kiln.source.projection.build_inputs] A build-input planner MUST accept only a completeness witness for a build, exact facts and layout facts of the same full source candidate, and a versioned build request naming its profile, targets, command, toolchain and environment. Under the Cargo compatibility profile it MUST require, at their own source paths, every file of each target package, of every workspace member Cargo loads and of their path-dependency closure, together with the build root's workspace manifest and lockfile and the Cargo configuration and toolchain files at the build root and its ancestors. Under the Nix compatibility profile it MUST refuse a view that omits any source file. Before any execution it MUST refuse unavailable or stale facts, relocated or generated entries, unknown or ambiguous targets, targets with several build roots, path dependencies outside the repository and every absent required input. An admitted plan MUST carry a BLAKE3 identity over the full candidate, the projection manifest and content, the profile, the required inputs with their roles and objects, the command, the toolchain and the environment, and evidence MUST apply only to a plan with that identity. A plan MUST NOT grant read, write, execution or publication authority and MUST NOT be reported as a successful build.

#### Scenario: A project view with its shared dependency is planned

r[kiln.source.projection.build_inputs.scenario.planned]
- GIVEN a two-project Cargo workspace with glob members, a shared path dependency, inherited workspace fields, a root lockfile and Cargo configuration
- WHEN a view holding one project, the shared dependency and the root files is planned for that project
- THEN the plan MUST list exactly those packages and root files with their roles, and name the workspace root as the build root

#### Scenario: Missing, relocated and unsupported inputs are refused

r[kiln.source.projection.build_inputs.scenario.refused]
- GIVEN unavailable layout facts, facts from another candidate, a relocated or generated entry, an unknown or ambiguous target, targets with several build roots, an absent dependency, lockfile, configuration or toolchain file, an absent literal member, a partly held glob member or a path dependency outside the repository
- WHEN the view is planned
- THEN the planner MUST refuse it with a diagnostic for each fault and plan nothing

#### Scenario: Restricted Nix views are refused

r[kiln.source.projection.build_inputs.scenario.nix]
- GIVEN a Nix flake with a declared path input and a sibling read that no input declares
- WHEN a view without the sibling and a view of the whole tree are planned
- THEN the restricted view MUST be refused and the whole-tree plan MUST include the declared path input

#### Scenario: Evidence follows the plan identity

r[kiln.source.projection.build_inputs.scenario.applicability]
- GIVEN evidence recorded for one plan
- WHEN the layout, a required input object, the toolchain, the command or the environment changes
- THEN the new plan MUST have another identity and the evidence MUST NOT apply to it

#### Scenario: A planned view is qualified through the materializer

r[kiln.source.projection.build_inputs.scenario.qualified]
- GIVEN the Cargo corpus committed with a working checkout and its planned views materialized through the projected materializer
- WHEN the declared toolchain matches and each planned command runs at the build root of an attempt overlay with a cleared environment and a private target directory, and the shared dependency is edited through one view
- THEN the complete view MUST pass its tests without changing its entries, a view whose code reads a file outside it MUST fail although the checkout holds that file, and the edit MUST select the dependent project outside the editing view
