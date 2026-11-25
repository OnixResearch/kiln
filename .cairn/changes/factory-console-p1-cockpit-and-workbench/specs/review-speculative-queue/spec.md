## ADDED Requirements

### Requirement: Visual Speculative Merge Train view and Cockpit status strip

r[kiln.review.queue.speculative.visual_train]
The Code Review workbench MUST expose a first-class `Merge Queue` tab rendering per-target speculative `Chain` predecessor graphs (`Window floor..ceiling`, active depth, `generation`, `root_target`, and retired `superseded_chains` with exact ejection/rebuild causes). The `1.1 Factory` Cockpit MUST display a compact 1-line-per-target Merge Train status strip linking directly into the Code Review `Merge Queue` tab or opening the item in the Slide-Over Inspector.

#### Scenario: Retired speculative chain explains predecessor failure

r[kiln.review.queue.speculative.visual_train.scenario.retired_chain]
- GIVEN a speculative merge train where predecessor `#1` failed CI and caused `#2` and `#3` to rebuild in a new generation
- WHEN the operator opens the `Merge Queue` tab in Code Review
- THEN the UI MUST render the active predecessor chain, window depth, and the retired `superseded_chains` entry stating the exact predecessor failure cause
