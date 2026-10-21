## MODIFIED Requirements

### Requirement: Bounded Unix server and reconciliation

r[kiln.aspen_host.transport] The host bridge MUST decode and encode bounded exact request frames, retain process-local operation acknowledgements, and reconcile without provider redispatch. The server MUST bound accepted connections and continue serving after a per-connection read, decode, encode, or response-write failure.

#### Scenario: Submit and reconcile agree

r[kiln.aspen_host.transport.accepted]
- GIVEN one valid request reaches the admitted socket
- WHEN submit completes and the client later reconciles its operation
- THEN both responses MUST bind the same profile, operation, run, and terminal observation
- AND a repeated submit MUST return replay without a second provider effect

#### Scenario: Frame or acceptance certainty fails

r[kiln.aspen_host.transport.rejected]
- GIVEN an oversized, malformed, stale, unsupported, incomplete, or conflicting frame
- WHEN the server handles the connection
- THEN it MUST reject without a provider effect
- AND a disconnect after possible acceptance MUST remain `Unknown`

#### Scenario: Client disconnect does not stop the host

r[kiln.aspen_host.transport.disconnect]
- GIVEN a client connects, sends a valid request, and disconnects before reading the response
- WHEN the server writes the response and the write fails
- THEN it MUST log the failure and consume one request slot
- AND later valid requests MUST still be served within the connection bound
