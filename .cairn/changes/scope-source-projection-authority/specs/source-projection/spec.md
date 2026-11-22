# Source projection delta

## ADDED Requirements

### Requirement: Closed source-view authority vocabulary

r[kiln.source.projection.authority.vocabulary] Kiln MUST retain a closed source-view vocabulary, `kiln.source-authority-coverage.v1`, separate from the protected implementation operations: discover, read content, read history, prepare workspace, modify source, create source, delegate and retrieve result. Each operation MUST map one-to-one to a reviewed `kiln/source/...` ability, an effect class and an attempt rule under which exactly the mutating operations require an attempt and fence. An unknown ability MUST map to no operation. No operation MUST imply another, and candidate submission and publication MUST reuse the protected implementation operations instead of joining this vocabulary.

#### Scenario: The coverage map is closed

r[kiln.source.projection.authority.vocabulary.scenario.closed]
- GIVEN the source coverage map
- WHEN it is validated
- THEN every operation MUST appear once with a unique `kiln/source/` ability and the mutation attempt rule
- AND a duplicate, missing, foreign-namespace or attempt-rule fault MUST be refused, and an unknown ability MUST resolve to no operation

#### Scenario: Reading implies nothing else

r[kiln.source.projection.authority.vocabulary.scenario.no_implication]
- GIVEN a grant naming one operation over the whole repository
- WHEN any other operation is checked
- THEN it MUST be refused; a reader MUST NOT share, edit, prepare, retrieve results or publish

### Requirement: Exact source-view requests

r[kiln.source.projection.authority.request] A source-view request MUST bind the operation's ability, repository, snapshot rule, the view binding it is made through, the origin scope, the payload digest and, where present, the attempt and its non-zero fence. A mutating operation without an attempt MUST be invalid. Changing any bound field MUST change the request identity.

#### Scenario: Every field is bound

r[kiln.source.projection.authority.request.scenario.binding]
- GIVEN one valid request
- WHEN any bound field changes, or a mutating request omits its attempt or fence
- THEN the identity MUST change, or validation MUST fail

### Requirement: Per-component effective scope

r[kiln.source.projection.authority.scope] An effective source scope MUST be built only by an authority adapter from a verified session, MUST NOT be deserializable, and MUST bind the holder, repository, snapshot rule, the exact admitted revision, canonical grants, expiry, delegation depth, authority and revocation generations and provenance into its identity. Its grants MUST be only those the port admitted request by request, narrowed to what the session's capability set confirms: a capability-set grant that was never individually admitted, such as a sibling the delegation chain rule did not check, MUST confer nothing. Path scope MUST be framed per literal component with an explicit subtree marker, so a grant for `a/b` never covers `a/bc`, and empty, dot, glob, separator and control components MUST be refused.

#### Scenario: Paths compare by component

r[kiln.source.projection.authority.scope.scenario.components]
- GIVEN a subtree grant for `crates/app` and an exact grant for one file
- WHEN `crates/apple`, a longer path below the file, or an unsafe component is checked
- THEN none MUST be covered or admitted

#### Scenario: The scope identity binds every field

r[kiln.source.projection.authority.scope.scenario.identity]
- GIVEN one admitted scope
- WHEN any bound field changes, apart from the order of its grants
- THEN the identity MUST change

#### Scenario: A capability set never extends an admission

r[kiln.source.projection.authority.scope.scenario.admitted_only]
- GIVEN a session whose capability set also reports a sibling grant the admitted requests did not cover
- WHEN its scope is built, joined with other admissions and used to restrict a view
- THEN the sibling MUST NOT be admitted, permitted, listed, looked up or presented

### Requirement: Source-view authority port

r[kiln.source.projection.authority.port] A source authority port MUST admit one exact request against current snapshot, generation, revocation and attempt facts, retain a live session and expose that session's effective scope, and MUST recheck the session before each use, discarding it on any denial. A session MUST bind one exact revision: a live view MUST follow its reference only under its admitted evolution and only through a fresh admission. A view handle presented as a resource MUST be refused. Decisions MUST be bounded and redacted.

#### Scenario: The verified port scopes each holder

r[kiln.source.projection.authority.port.scenario.verified]
- GIVEN the agent's grant over its crate in the shared corpus
- WHEN it reads its crate, reads the hidden dependency or a longer-named sibling, delegates, reads history, edits without or with an attempt, acts for a superseded attempt or after expiry, or an unknown holder asks
- THEN only the granted read and the attempt-bound edit MUST be admitted, each other request with its own closed reason

#### Scenario: Revalidation follows current authority

r[kiln.source.projection.authority.port.scenario.revalidate]
- GIVEN an admitted live session
- WHEN the revocation view or authority generation changes, the reference moves, the holder is revoked, or another payload is presented
- THEN revalidation MUST deny and discard the session, and only a fresh admission along the admitted evolution MUST yield a new scope

#### Scenario: Live views evolve only as admitted

r[kiln.source.projection.authority.port.scenario.evolution]
- GIVEN exact, fast-forward and rewrite snapshot rules
- WHEN a reference moves forward, is rewritten or names another reference, or a child rule is compared with its parent
- THEN each rule MUST admit only its own evolution, and no child rule MUST name revisions outside its parent

### Requirement: Explicit local source compatibility

r[kiln.source.projection.authority.compatibility] The local-operator source profile MUST admit only the explicitly selected operator, over the repositories and operations its full-source grant names. A missing profile, a verified credential and another operator MUST deny; there MUST be no role path; and a grant naming delegation MUST be refused.

#### Scenario: Only the explicit operator grant admits

r[kiln.source.projection.authority.compatibility.scenario.explicit]
- GIVEN a local grant for discovery and reading of one repository
- WHEN the operator reads, or a missing profile, a verified credential, another operator, another repository, an ungranted operation or a handle is presented
- THEN only the operator's read MUST be admitted, with local-operator provenance and a root-only scope

### Requirement: Restricted audience views

r[kiln.source.projection.disclosure.restricted] A restricted view MUST be derived from the trusted view, the holder's current scope, a frozen view ceiling and the shell's handle key, so that the effective view is the definition's selection intersected with the ceiling, the admitted snapshot and the current grant. Entries MUST be filtered by their native origin before anything is counted, truncated, paged or turned into coverage; an alias MUST be judged by its origin. A restricted view MUST NOT be deserializable or constructible outside the restriction, and its presentation MUST omit every trusted binding, native digest and hidden path.

#### Scenario: Three audiences see three views

r[kiln.source.projection.disclosure.restricted.scenario.audiences]
- GIVEN the shared corpus with maintainer, project-reader and agent views over one snapshot
- WHEN each is restricted
- THEN each MUST show exactly its expected entries, statuses and Cairn items, with the dependency identity only for full source

#### Scenario: Selection, ceiling, snapshot and grant all bound the view

r[kiln.source.projection.disclosure.restricted.scenario.filter_first]
- GIVEN a full-source holder through a narrower ceiling, an expired scope, another revision or repository, or a scope without discovery or read
- WHEN the view is restricted
- THEN it MUST show only the ceiling's entries, or refuse with one public shape

#### Scenario: Forbidden-only changes are invisible

r[kiln.source.projection.disclosure.restricted.scenario.forbidden_invariance]
- GIVEN generated snapshots that differ only in entries or dependency identities the holder may not see
- WHEN each is restricted for the same grant
- THEN the visible entries, coverage and visible-content identity MUST be equal

#### Scenario: Presentations carry nothing trusted

r[kiln.source.projection.disclosure.restricted.scenario.presentation]
- GIVEN restricted views of the reader and agent
- WHEN they are presented and debug-printed
- THEN no manifest, binding, scope, definition or repository identity, revision, native object, hidden path or key MUST appear

#### Scenario: Restricted views cannot be forged

r[kiln.source.projection.disclosure.restricted.scenario.unforgeable]
- GIVEN serialized data claiming to be a restricted view, context or effective scope
- WHEN it is deserialized
- THEN compilation MUST fail

### Requirement: Keyed handles, cursors and caches

r[kiln.source.projection.disclosure.handles] A view binding MUST bind the repository, revision, definition, trusted manifest, ceiling, audience, effective scope and both generations; its identity MUST key every handle, cursor and cache. Handles and cursors MUST be BLAKE3 hashes keyed by a shell-held key and MUST confer no authority. Every use MUST recheck the binding against the holder's current scope, and resolving a handle MUST recheck the operation over its origin.

#### Scenario: Handles are keyed and scoped

r[kiln.source.projection.disclosure.handles.scenario.keyed]
- GIVEN equal-byte files, an alias and its origin, two holders and two keys
- WHEN handles are compared, unkeyed digests or another holder's handle are resolved, or a handle is presented at the port
- THEN every handle MUST be distinct, and none of the guesses MUST resolve or be admitted

#### Scenario: Cursors and caches recheck current authority

r[kiln.source.projection.disclosure.handles.scenario.cursor]
- GIVEN a paged reader view
- WHEN a broader holder's cursor or cached view, another key, a forged position, a revoked or regenerated scope or expiry is used
- THEN the page MUST be refused as stale

#### Scenario: Narrowing keeps hashes and invalidates bindings

r[kiln.source.projection.disclosure.handles.scenario.narrowing]
- GIVEN a reader whose grant is narrowed
- WHEN the view is restricted again
- THEN canonical manifest and content identities MUST be unchanged, while the binding, handles and cached view MUST be invalidated

### Requirement: Uniform unavailability and distinct coverage

r[kiln.source.projection.disclosure.unavailable] A missing, forbidden or stale object MUST share one public unavailable shape while the trusted owner keeps the reason. The trusted enumeration behind a restricted view MUST be made over the holder's scoped selection: the ceiling's paths intersected with the paths its current grants let it discover or read, covering the repository root only for a full-source holder. An enumeration recorded for another selection MUST be refused as stale, and one holding an entry outside its selection MUST be refused. A view's coverage MUST be complete only when every authorized entry is listed, and otherwise truncated at the number of entries listed, whether the holder-scoped enumeration or the view's own bound stopped the listing; it MUST NOT depend on hidden entries or reveal global counts. A holder that may discover a path MUST be told whether it is present, unrequested by the definition, known absent, unknown because the holder-scoped enumeration was truncated, or beyond the view's bound. Missing and denied dependencies MUST be the same unavailable state.

#### Scenario: Missing and forbidden probes look the same

r[kiln.source.projection.disclosure.unavailable.scenario.probe]
- GIVEN a hidden path, a missing path, an alias of a hidden origin and origin probes outside discovery
- WHEN they are looked up
- THEN each MUST be refused with the same serialized shape

#### Scenario: Coverage states are distinct

r[kiln.source.projection.disclosure.unavailable.scenario.coverage]
- GIVEN complete, holder-scoped truncated and bounded views
- WHEN the views are presented and discoverable paths are looked up
- THEN each view's coverage MUST be complete only when every authorized entry is listed and truncated otherwise, and present, unrequested, absent, unknown and truncated MUST be distinct lookup results, with unknown only for a truncated holder-scoped enumeration

#### Scenario: Enumeration is scoped to the holder

r[kiln.source.projection.disclosure.unavailable.scenario.scoped]
- GIVEN a snapshot where hidden entries would push an enumeration of the whole view selection past the source adapter's bound
- WHEN the reader's view is enumerated over its scoped selection, or restricted from an enumeration of another selection or one holding entries outside its selection
- THEN the reader's view MUST be complete with every authorized entry, a truncation of the holder-scoped enumeration MUST surface as truncated coverage and unknown lookups, and the other enumerations MUST be refused

#### Scenario: Denied dependencies are only unavailable

r[kiln.source.projection.disclosure.unavailable.scenario.dependencies]
- GIVEN a required dependency that the owner denied, or that is missing
- WHEN the agent's view is restricted
- THEN both MUST present the same unavailable dependency without identity, and neither view MUST be complete

### Requirement: Restricted Cairn context

r[kiln.source.projection.disclosure.cairn] Cairn context MUST be disclosed through `.cairn/changes/<id>/**` path authority and no separate resource family. A complete observation MUST bind its repository and exact revision and admit items only under their own change path with unique ids and known prerequisites. A restricted context MUST be derived only from an observation of the view's repository at its exact revision, MUST bind the audience's view binding and the exact observation, MUST show only items the holder may discover with text only under read authority, and MUST NOT convert into, or be accepted as, a complete observation. A hidden prerequisite of a visible item MUST appear only as an opaque handle; its status MUST be shown only when the holder may discover `.cairn/changes/<id>/` of the prerequisite's change, and otherwise MUST be `unavailable`, which still blocks the item that depends on it.

#### Scenario: A hidden prerequisite stays an obligation

r[kiln.source.projection.disclosure.cairn.scenario.hidden_prerequisite]
- GIVEN a visible task depending on a hidden open task, and observations of another repository or revision
- WHEN the reader's, agent's and maintainer's contexts are delivered, and a holder whose ceiling hides the prerequisite but who may discover its change receives one
- THEN the reader MUST see the task blocked by an opaque handle with an `unavailable` status and without its name or text, the holder that may discover the change MUST see the prerequisite's real status, the agent MUST see no text, the maintainer MUST see every item, each context MUST be bound to its own audience, and a foreign observation MUST be refused as stale

#### Scenario: Observations stay change-scoped

r[kiln.source.projection.disclosure.cairn.scenario.observation]
- GIVEN items outside their change path, with unknown prerequisites or duplicate ids
- WHEN the observation is admitted
- THEN it MUST be refused, and input order MUST NOT change its identity

#### Scenario: A restricted context is never complete

r[kiln.source.projection.disclosure.cairn.scenario.not_complete]
- GIVEN a restricted context
- WHEN it is passed or converted where a complete observation is required
- THEN compilation MUST fail

### Requirement: Attenuated sharing

r[kiln.source.projection.sharing.attenuation] Sharing MUST be an operation separate from reading. A share MUST name one exact recipient DID, origin path scopes, operations, a snapshot rule, an expiry and a depth, and MUST be admitted only when every path lies in the sharer's grants and the view ceiling, the sharer holds `delegate` over every path with depth remaining, the child depth is below the sharer's, a delegating child keeps depth, every operation lies in the ceiling and the sharer's grant, the snapshot lies within the sharer's, and the share does not outlive the sharer. A view reference or handle alone MUST confer nothing.

#### Scenario: Corpus shares attenuate or refuse

r[kiln.source.projection.sharing.attenuation.scenario.corpus]
- GIVEN the corpus share cases
- WHEN each is admitted
- THEN the attenuated read MUST be admitted and the reader's share, alias, out-of-ceiling path, navigation edit, outliving, depth and tag cases MUST be refused with their named reasons

#### Scenario: Malformed and expired shares are refused

r[kiln.source.projection.sharing.attenuation.scenario.bounds]
- GIVEN wildcard or malformed recipients, empty paths or operations, a past expiry, excessive depth, an expired sharer, or a delegating child without depth
- WHEN each is admitted
- THEN each MUST be refused

#### Scenario: A child never exceeds its parent

r[kiln.source.projection.sharing.attenuation.scenario.property]
- GIVEN every combination of candidate path, operation set, expiry and depth for two sharers and three ceilings
- WHEN each is admitted
- THEN every admitted grant MUST lie within the sharer's grants, delegation and the ceiling, and no plan MUST outlive or match the sharer's depth

#### Scenario: A reference confers nothing

r[kiln.source.projection.sharing.attenuation.scenario.reference]
- GIVEN a reader's view reference and handles passed to a holder without a grant
- WHEN that holder presents them
- THEN the port MUST deny, and the reader MUST NOT turn its read into a share

### Requirement: Frozen view ceilings

r[kiln.source.projection.sharing.ceiling] A view ceiling MUST be derived from an admitted definition as the selection root with each include pattern's literal components, exact for a literal pattern and a subtree otherwise, together with the operations the profile can ever admit: navigation never prepares or mutates, and only a writable view modifies or creates. A share MUST bind the ceiling it was admitted under, so a later definition edit, alias or dependency cannot widen it.

#### Scenario: View edits and aliases stay within the ceiling

r[kiln.source.projection.sharing.ceiling.scenario.edits]
- GIVEN a reader view whose definition is edited to add documentation and alias it into the reader's crate
- WHEN the reader's and the full-source holder's views are restricted under the edited and the frozen ceilings
- THEN the alias MUST stay hidden from the reader, and under the frozen ceiling the edit MUST add nothing

#### Scenario: Expanding a saved view does not expand a share

r[kiln.source.projection.sharing.ceiling.scenario.saved_view]
- GIVEN an issued share and a later expanded saved view
- WHEN the recipient reads outside its share or restricts the expanded view under the frozen or new ceiling, or shares further
- THEN it MUST see only its shared paths and share only within them at lower depth and lifetime
