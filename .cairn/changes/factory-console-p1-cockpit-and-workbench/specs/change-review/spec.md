## ADDED Requirements

### Requirement: Three-pane Code Review workbench, inline threads, jj stack rail, and list CI summary

r[kiln.review.workbench_three_pane]
The Code Review view MUST provide a 3-pane workbench inside `.page-frame-wide` (`110rem`): a sticky `16rem` file-tree and noise-fold rail, a central diff table rendering anchored discussion threads inline beneath their target lines, and a `23rem` 4-tab sticky aside (`Checklist & Votes`, `CI & Previews`, `Queue & Stack`, `History & Ops`). Switching to side-by-side (`split`) diff mode or viewports narrower than `90rem` MUST auto-collapse the left file rail into a toggleable drawer. An expandable `jj` Stack Rail MUST sit above the diff displaying stack order, `jj_change_id`, `jj_stack.removed` warnings, and 1-click interdiff. `GET /api/code-review` MUST include a lightweight `ci_summary` per change so `drawList()` displays CI, approval, queue, and stack readiness without N+1 detail requests.

#### Scenario: Unified vs split diff adapts rail without overflow

r[kiln.review.workbench_three_pane.scenario.adaptive_panes]
- GIVEN a code review with anchored diff threads opened at `>= 90rem`
- WHEN the operator reviews in `unified` mode and then switches to `split` mode
- THEN `unified` mode MUST show all 3 panes with inline anchored threads
- AND switching to `split` mode MUST auto-collapse the left file rail into a toggleable drawer so side-by-side diffs remain readable without horizontal document overflow
