## ADDED Requirements

### Requirement: Pending host content index publication

r[kiln.aspen_runtime.content_index_retry] The host content port MUST return a successful put receipt only after every pending digest-to-manifest index update reaches payload synchronization, exact same-parent replacement and parent synchronization through the reviewed durable-publication mechanism. An equal retry MUST NOT bypass an earlier publication failure. Reopened visible mappings MUST be reconciled before another publication acknowledgement. Content verification MUST NOT imply effect completion or authority.

#### Scenario: Persistent publication fault

r[kiln.aspen_runtime.content_index_retry.scenario.fault]
- GIVEN a real host content store with a create, write, payload-sync, rename or parent-sync failure
- WHEN the same exact put is retried while the failure remains
- THEN every attempt MUST return failure and MUST NOT acknowledge the pending mapping

#### Scenario: Publication recovery survives reopen

r[kiln.aspen_runtime.content_index_retry.scenario.recovery]
- GIVEN a failed pending exact put
- WHEN the fault is removed and the same put succeeds
- THEN exact verified readback MUST survive dropping and reopening the port

#### Scenario: Published entries survive failed addition

r[kiln.aspen_runtime.content_index_retry.scenario.preserved]
- GIVEN an index with previously acknowledged exact content
- WHEN a new addition fails publication or an interrupted stage obstructs publication
- THEN previously published entries MUST remain resolvable and no unowned stage MUST be deleted

#### Scenario: Parent barrier remains uncertain

r[kiln.aspen_runtime.content_index_retry.scenario.barrier]
- GIVEN rename succeeded but parent synchronization failed
- WHEN the port reports the result or is reopened
- THEN the visible index MUST NOT be rolled back or treated as durable success before reconciliation, and evidence MUST NOT claim storage-device or power-loss durability
