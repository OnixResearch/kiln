# Workflow Presentation Specification

## Purpose

Defines the `workflow-presentation` capability.

## Requirements

### Requirement: Readable and truthful workflow inspection

r[kiln.dashboard.workflow.presentation]

The dashboard SHALL present admitted workflow dependencies as readable stages with textual node states, bounded fit/zoom and exact-node inspection. It SHALL retain the accessible node list, URL-selected node, explicit unknown timings and read-only authority. Dependency stages SHALL NOT imply execution chronology, readiness, placement or success.

#### Scenario: Inspect an admitted graph

- GIVEN admitted nodes and an acyclic dependency graph
- WHEN an operator selects a node or adjusts the diagram
- THEN stages and edges remain readable, immediate relationships are highlighted, and prerequisite/dependent navigation and logs preserve the exact node selection

#### Scenario: Historical or malformed topology

- GIVEN missing edges, duplicate node IDs, unknown endpoints or a cycle
- WHEN the workflow view renders
- THEN topology is explicitly unavailable instead of invented, while retained node records and unknown values remain visible

#### Scenario: Responsive live inspection

- GIVEN long node names, branching dependencies, keyboard focus, zoom and a scrolled diagram
- WHEN the same run refreshes or is inspected on a narrow viewport
- THEN cards do not overlap, the page does not overflow horizontally, selection/focus remain exact, and clearing the session retires all private workflow content
