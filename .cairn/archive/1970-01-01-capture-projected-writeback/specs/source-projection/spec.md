# Source projection delta

## ADDED Requirements

### Requirement: Captured and sealed projected edits

r[kiln.source.projection.writeback] A capture MUST accept only a completeness witness for a write, its admitted definition, the overlay's workspace pin, the shell's observation of that overlay and the attempt's current fence and source facts. It MUST map each changed, created or removed overlay file back through the manifest and mounts to one typed source operation (edit, create or delete) that names its expected base object. Before any effect it MUST refuse a moved source, a superseded fence, a pin for another view, aliases that disagree, a new file outside the declared creation destinations, a new file that maps to no source path or to several, a new file outside the selection or over a source file the view does not hold, and unsupported entries. A sealed candidate MUST carry a BLAKE3 identity over its base, definition, manifest, pin and operations. A sealing shell MUST write exactly the captured bytes, refuse an overlay that changed after capture, and build the candidate from the base tree so that every path outside the operations keeps its object and mode. Capture and sealing MUST NOT move a reference or grant publication authority.

#### Scenario: A shared edit round-trips into one source update

r[kiln.source.projection.writeback.scenario.round_trip]
- GIVEN a writable view of one project and a shared directory mounted twice, materialized into a pinned attempt overlay
- WHEN the shared file is edited at both destinations, a file is created under a creation destination and another is removed, and the overlay is captured and sealed
- THEN the candidate MUST hold exactly one edit, one create and one delete, every other path MUST keep its base object and mode, a view resolved at the candidate MUST show the new version, and another attempt's pinned overlay MUST keep the old one

#### Scenario: Equal bytes at two origins stay independent

r[kiln.source.projection.writeback.scenario.independent]
- GIVEN two source files with equal bytes at different origins, one of them in the view
- WHEN the one in the view is edited and the overlay is captured
- THEN only that origin MUST change

#### Scenario: Ambiguous and contradictory edits are refused

r[kiln.source.projection.writeback.scenario.refused]
- GIVEN an alias edited at one destination only, a new file outside every creation destination, a new file that two mounts map to different source paths, a new file over a source file the view does not hold, or a symbolic link
- WHEN the overlay is captured
- THEN the capture MUST be refused with a diagnostic for each fault and nothing MUST be sealed

#### Scenario: Stale and superseded captures are refused

r[kiln.source.projection.writeback.scenario.stale]
- GIVEN source facts at another revision, a current fence beyond the pin's, a pin for another manifest, or a completeness witness for a build
- WHEN the overlay is captured
- THEN the capture MUST be refused

#### Scenario: A sealed candidate is immutable

r[kiln.source.projection.writeback.scenario.sealed]
- GIVEN a candidate that was captured and sealed
- WHEN the worker writes to the overlay again
- THEN sealing the earlier capture MUST be refused, a new capture MUST have another identity, and the earlier sealed commit MUST be unchanged
