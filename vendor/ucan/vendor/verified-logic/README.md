# Trellis

Natural, composable verified-logic building blocks for reliable systems.
Trellis provides a lightweight structure for growing reusable proofs, specs,
and runtime code that stays mechanically checked.

The house style is a `spec fn` (mathematical definition) plus an
`exec fn` whose `ensures result == spec(args)` clause is verified by
[Verus](https://verus-lang.github.io/verus/guide/). The specs are embedded
directly in `src/`; tracked exceptions such as `src/ssa_construction.rs`
are listed in `openspec/specs/verification-gaps/spec.md` so any weaker
bridge stays explicit. Requirement coverage is tracked by
[Tracey](https://tracey.bearcove.eu/).

**10611 Verus proofs · 4824 tests · 2005 requirements · 604 modules · 16 trusted/proof-gap boundaries**

## How it works

```rust
verus! {

// Mathematical definition (ghost code, erased at compile time)
pub open spec fn abs_diff_spec(a: u64, b: u64) -> u64 {
    if a >= b { (a - b) as u64 } else { (b - a) as u64 }
}

// Runtime implementation — Verus checks `ensures` against the body
pub fn abs_diff(a: u64, b: u64) -> (result: u64)
    ensures result == abs_diff_spec(a, b)    // ← mechanically checked
{
    if a >= b { a - b } else { b - a }
}

// Machine-checked proof of a property
proof fn abs_diff_symmetric(a: u64, b: u64)
    ensures abs_diff_spec(a, b) == abs_diff_spec(b, a)
{}

} // verus!
```

## Modules

### Composite primitives

The `compose-verified-primitives` OpenSpec change landed twenty thin composition
facades. Each module has a public facade, paired spec predicates, proof bridge,
and positive / negative runtime evidence. Headline counts above are from fresh
validation commands captured in the change evidence.

| Module | Composition purpose | Status |
|---|---|---|
| `durable_job_executor` | durable, idempotent, retry-aware job execution | verified |
| `distributed_lock` | stale-holder-safe lease writes with fencing tokens | verified |
| `linearizable_read_gate` | Raft read-index / lease-read admission | verified |
| `safe_snapshot_replication` | complete, integrity-checked snapshot installation | verified |
| `crash_safe_page_cache` | WAL-backed dirty-page recovery | verified |
| `verified_lsm_ingest` | flush and compaction-safe LSM ingest | verified |
| `serializable_txn_guard` | MVCC / dependency / 2PL commit admission | verified |
| `web_of_trust_ucan_authorizer` | UCAN authorization plus rooted web-of-trust policy | verified |
| `policy_decision_engine` | delegation, containment, scope, pattern, and trust allow/deny | verified |
| `blocking_bounded_channel` | bounded queue capacity plus futex/backoff waiting | verified |
| `safe_reclamation_domain` | RCU, epoch, hazard-pointer, and ref-count reclamation | verified |
| `cache_coherent_memory_region` | coherence, prefetch, store-buffer, and write-policy invariants | verified |
| `verified_compiler_pipeline` | raw-IR analysis and optimization preservation | verified |
| `backend_frame_allocator` | register/spill stack allocation conflict freedom | verified |
| `dependency_scheduler` | DAG-safe, fair, capacity-bounded job scheduling | verified |
| `raft_membership_manager` | safe config changes, learner promotion, leadership transfer | verified |
| `exactly_once_ledger_transfer` | replay-safe balanced transfer posting | verified |
| `sel4_invocation_step` | bounded seL4 invocation transitions preserving invariants | verified |
| `nix_eval_closure_policy` | Nix/Nickel merge plus reachable closure admission | verified |
| `chaoscontrol_exploration_core` | deterministic, oracle-checked exploration steps | verified |
| `integration_adapters` | deterministic user-facing adapters over verified consensus, UCAN, Nix/Nickel, and WAL primitives | adapter |

### Core arithmetic and data structures

| Module | What it verifies |
|---|---|
| `arithmetic` | abs diff, saturating add/sub, checked mul, gcd, div ceil |
| `bitwise` | power-of-two, bit get/set/clear/toggle |
| `bitfield` | multi-bit field extraction and insertion in u64 words |
| `bitmap` | fixed-size bit set over u64 words |
| `bounds` | range predicates, clamping |
| `collections` | sorted insert, binary search, merge sorted, partitioning, find, count |
| `conversion` | checked narrowing (u32/u16/u8), signed↔unsigned |
| `integer` | parity, alignment, log2, next-power-of-two |
| `interval` | half-open interval contains, overlap, split, adjacent, merge |
| `option` | verified Option combinators (map, and_then, or_else, flatten, zip, filter) |
| `ordering` | min, max, median, sign, compare-and-swap |
| `result` | verified Result combinators (map_ok, map_err, and_then, or_else) |
| `ring_buffer` | FIFO queue with power-of-two capacity |
| `sequence` | monotonic sequence generator with batch reservation |
| `slice` | chunk count, prefix/suffix equality, all-equal, mismatch |

### Rust async primitives

| Module | What it verifies |
|---|---|
| `async_task_state` | finite pending/ready/completed/cancelled task lifecycle admission with terminal-transition rejection proofs and focused positive/negative tests |
| `task_waker` | finite empty/registered/notified/taken generation-tagged waker-slot admission plus no-lost-wake bridge predicates that avoid callback/fairness claims |
| `oneshot_channel` | finite open/sent/received/closed/cancelled single-slot channel admission plus receiver readiness/waker-notification composition |
| `drop_guard` | finite armed/disarmed/cleaned exactly-once cleanup guard admission with repeated-cleanup rejection evidence |

### Circuit primitives

| Module | What it verifies |
|---|---|
| `r1cs_constraint` | bounded single-row R1CS shape, modular dot product, and satisfaction admission without multi-row/proving-system claims |
| `r1cs_bit_decomposition` | bounded two-bit decomposition over boolean R1CS witness slots without arbitrary-width range-check/proving-system claims |
| `r1cs_lookup` | bounded lookup-table membership over active R1CS witness/table prefixes without PLONK lookup/proving-system claims |
| `r1cs_boolean` | bounded booleanity row encoding and satisfaction for witness-slot bit constraints without range-check/proving-system claims |
| `r1cs_gates` | bounded multiplication, addition, and constant gate adapters into R1CS rows without circuit-builder/proving-system claims |
| `r1cs_system` | bounded multi-row R1CS system shape and shared-witness satisfaction without proving-system claims |
| `r1cs_public_inputs` | bounded public-input binding and R1CS statement satisfaction without proving-system claims |

### Raft consensus

| Module | What it verifies |
|---|---|
| `election` | leader election state machine |
| `election_init` | candidate election initiation |
| `election_jitter` | election jitter via Knuth multiplicative hash |
| `election_timer` | election timer reset conditions |
| `election_safety_proof` | at most one leader per term (system-level) |
| `pre_vote` | pre-vote protocol |
| `pre_vote_tally` | pre-vote response counting |
| `pre_vote_lease` | PreVote + CheckQuorum interaction |
| `priority_election` | priority-based leader election with delay and yield |
| `vote_tally` | vote response counting |
| `ballot` | vote tracking |
| `epoch` | lexicographic (term, index) comparison |
| `quorum` | majority threshold, intersection theorem, fault tolerance |
| `heartbeat` | heartbeat send/receive primitives |
| `check_quorum` | leader liveness via active peer counting |
| `term_authority` | step-down on higher term |
| `log` | append-only log with Log Matching Property |
| `log_conflict` | log conflict detection and resolution |
| `log_compaction` | prefix truncation for snapshot-based compaction |
| `log_invariant_inductive` | inductive proof of LMP preservation across transitions |
| `append_entries_build` | AppendEntries RPC construction |
| `append_entries_check` | AppendEntries consistency check |
| `append_response` | AppendEntries response dispatch |
| `match_index` | match/next index tracking for replication |
| `commit_advance` | commit index advancement |
| `commit_notify` | commit notification dispatch |
| `fast_backtrack` | accelerated log backtracking |
| `replication_mode` | per-peer FSM (Probe/Replicate/Snapshot) |
| `replication_pipeline` | pipelined replication flow control |
| `follower_commit` | follower commit index update |
| `follower_read` | follower linearizable reads via leader confirmation |
| `follower_redirect` | follower redirect primitives |
| `request_forward` | follower→leader write forwarding |
| `read_index` | read index protocol for linearizable reads |
| `lease_read` | lease-based reads |
| `noop_commit` | no-op entry for leader commitment |
| `install_snapshot` | snapshot installation |
| `snapshot_compaction` | snapshot creation and compaction trigger |
| `snapshot_transfer` | chunked snapshot transfer |
| `snapshot_integrity` | dual-hash integrity scheme |
| `config_change` | configuration change state machine |
| `joint_consensus` | joint consensus for membership changes |
| `voter_set` | voter set promotion |
| `learner_change` | learner add/remove primitives |
| `learner_readiness` | learner catch-up readiness |
| `leadership_transfer` | leadership transfer protocol |
| `witness` | witness replica primitives (vote without full log) |
| `bounded_peers` | bounded peer collection for bootstrap |
| `node_classify` | node classification for cluster reporting |
| `persistent_state` | crash recovery state management |
| `client_routing` | client request routing |
| `client_session` | exactly-once session tracking |
| `entry_type` | log entry type routing |
| `last_applied` | state machine application tracking |
| `linearizability` | linearizability specification |
| `state_machine_safety` | state machine safety theorem |
| `leader_append_only` | leader append-only invariant |
| `leader_completeness` | leader completeness theorem |
| `leader_completeness_ghost` | leader completeness with ghost state |
| `ghost_history` | ghost vote/provenance records for inductive proofs |
| `raft_step` | step decomposition for modular invariant proofs |
| `raft_timing` | timeout configuration validators |
| `trace_relation` | Raft internal state → client-visible trace |

### Other consensus protocols

| Module | What it verifies |
|---|---|
| `paxos` | single-decree Paxos acceptor: ballot ordering, prepare/accept, promise safety |
| `quepaxa` | finite deterministic QuePaxa proposal ordering, ISR transitions, abstract propagation, phase guards, fast path, and hedging metadata |
| `quorum_cert` | quorum certificate aggregation, vote counting, threshold checks |
| `view_change` | view-based reconfiguration: consecutive views, leader rotation |

### Capability authorization

| Module | What it verifies |
|---|---|
| `access_mode` | access mode partial order (Read ⊂ Full) |
| `authorize` | authorization composition |
| `containment` | delegation containment |
| `delegation` | delegation chain validation |
| `glob` | glob pattern matching with wildcards |
| `pattern` | set-membership pattern matching |
| `scope` | scope coverage, compound scoping |
| `subsumption` | variant subsumption |
| `token_timestamp` | UCAN token timestamp validity |

### Language and computation admission

| Module | What it verifies |
|---|---|
| `ability_effect` | finite ability requirements, handler discharge, and sandbox attenuation |
| `algebraic_effect` | finite algebraic-effect signatures, rows, summaries, handlers, and single-use continuations |
| `content_addressed_code` | content-addressed definition identity, dependency closure, and recursive group admission |
| `remote_computation` | remote computation dependency, ability, and resource admission |
| `semantic_refactoring` | immutable name movement, compatible propagation, and unresolved-dependent admission |
| `typed_durable_reference` | typed durable references, unpersist gates, and migration admission |

### Memory management

| Module | What it verifies |
|---|---|
| `buddy` | block sizing, buddy addressing, alignment, coalescing, order-for-size |
| `page` | virtual address decomposition, frame numbers, page alignment |
| `region` | address region contains, disjoint, adjacent, merge, split |
| `slab` | slot index/offset/address, generation handles, slab capacity |
| `watermark` | bump allocation with remaining/fits/alloc/reset |
| `heap_index` | binary heap parent/left/right child index arithmetic |

### Memory allocator

| Module | What it verifies |
|---|---|
| `alloc_token` | allocation lifecycle FSM: free → allocated → freed |
| `size_class` | size class bucketing, waste bounds, delegates to buddy order |
| `free_list` | intrusive free list: LIFO push/pop, no-cycle, containment |
| `split_coalesce` | buddy split/coalesce sequences with block conservation |
| `alloc_bitmap` | generational bitmap allocation, double-free detection |
| `arena` | region-based bump allocation with collective free |
| `pool` | fixed-size slab pool: alloc/free with bounded capacity |
| `segregated_free` | per-size-class segregated free lists |
| `coalesce_policy` | immediate vs deferred coalescing policies |
| `fragmentation` | external/internal fragmentation metrics with formal bounds |

### Concurrency primitives

| Module | What it verifies |
|---|---|
| `lock_ordering` | lock acquisition order, deadlock detection, cycle freedom via strict total order |
| `epoch_reclaim` | epoch-based reclamation: three-epoch cycling, pin/unpin, safe-to-reclaim gap proof |
| `hazard_pointer` | hazard pointer protocol: protect/unprotect, no-use-after-free, reclaimable scan |
| `seqlock` | sequence lock: even/odd counter, valid read detection, writer versioning |
| `ref_count` | reference counting: strong/weak, dealloc trigger, upgrade conditions |
| `work_stealing` | work-stealing deque: push/pop/steal bounds, circular buffer index |
| `barrier` | thread barrier: arrive/release, generation counter, leader detection |
| `spin_backoff` | spin-wait backoff: two-phase (spin then yield), phase state machine |
| `rcu` | read-copy-update: grace periods, quiescent state, safe reclamation |
| `mpsc_bounds` | MPSC channel bounds: occupancy, send/receive, capacity, close/drain |
| `futex_state` | futex wait/wake FSM: idle/waiting/notified, notify idempotence |
| `parking_lot_core_model` | finite key-to-waiter queues with uniqueness, requeue, and bucket metadata validity |
| `parking_lot_raw_mutex` | compact raw-mutex locked/parked-bit acquisition and release state machine |
| `parking_lot_condvar_model` | condition-variable association, notify, timeout, and no-spurious requeue semantics |
| `parking_lot_rwlock_model` | packed reader/writer/upgradable RwLock state transitions and fairness gates |
| `parking_lot_contracts` | parking-lot fairness, timeout, reentrant mutex, Once, raw-lock, and deadlock contracts |
| `ticket_lock` | FIFO-fair spinlock: ticket/serving counters, fairness, bounded waiters |
| `cas_loop` | CAS retry loop modeling: ABA detection via pack/unpack, retry bounds |
| `bounded_mpmc` | bounded MPMC ring buffer: head/tail, lap-based empty/full, occupancy |
| `memory_order` | memory ordering lattice: Relaxed ⊏ Acquire/Release ⊏ AcqRel ⊏ SeqCst |
| `cache_hierarchy` | inclusive/exclusive/non-inclusive L1/L2 invariants, back-invalidation, capacity, and snoop scope |
| `directory_coherence` | single-line directory protocol with sharer bit-vectors, targeted invalidations, exclusive-owner grants, eviction notifications, and MESI-compatible embedding |
| `cache_prefetch` | architecturally invisible prefetch hints with MESI-compatible cache effects and dirty-victim writeback |
| `store_buffer` | fixed-capacity store queue with youngest-match forwarding, FIFO drain, and fence-empty semantics |
| `tlb_shootdown` | page-local TLB shootdown: initiator self-invalidation, monotone acknowledgments, completion freshness, disjoint-page isolation, and range coverage |
| `semaphore` | counting semaphore: acquire/release permits, capacity bounds |
| `two_phase_lock` | two-phase locking: growing/shrinking phase discipline |
| `lease_mutex` | lease-based distributed mutual exclusion with fencing tokens |

### Financial accounting

| Module | What it verifies |
|---|---|
| `balance` | debit/credit with overflow guards |
| `double_entry` | double-entry bookkeeping invariants |
| `escrow` | fund/release/refund/dispute state machine with conservation |
| `fee` | basis-point fee computation, monotonicity, conservation |
| `journal` | append-only transaction journal with running balances |
| `ledger` | multi-account ledger with total conservation theorem |
| `settlement` | bilateral netting with positions-sum-zero theorem |
| `transfer` | value transfer between accounts, duplicate detection |

### Storage engine primitives

| Module | What it verifies |
|---|---|
| `btree_balance` | B-tree node fill percentage, split/merge sizing, underfull detection |
| `btree_insert` | B-tree node insert: sorted position, split on overflow |
| `btree_delete` | B-tree node delete: key removal, merge/rebalance on underflow |
| `lsm_level` | LSM level target sizing, level count, space/write amplification |
| `wal_segment` | WAL entry sequencing, segment space, truncation, contiguity |
| `wal_recovery` | WAL crash recovery: replay range, idempotent replay, checkpoint |
| `compaction_fence` | log compaction coordination with replication, safe compact index |
| `page_checksum` | Fletcher-style two-sum checksum, torn-write detection |
| `free_space_map` | free-space bitmap tracking: allocate/free, count, find-first-free |
| `write_buffer` | write buffer (memtable): size tracking, flush threshold, swap |
| `write_policy` | write-back/write-through dirty invariants, eviction, flush, read equivalence |
| `block_cache` | block cache: occupancy, eviction, dirty tracking, write-back |
| `set_associative_lookup` | cache address decomposition, per-set tag lookup, duplicate-tag checks |

### Distributed systems

| Module | What it verifies |
|---|---|
| `causal_order` | vector-clock-based causal ordering |
| `circuit_breaker` | circuit breaker state machine |
| `consistent_hash` | consistent hash ring lookup |
| `consistent_snapshot` | consistent cut: vector-clock-based cut validity, monotonicity |
| `crdt` | G-Counter, PN-Counter, LWW-Register |
| `distributed_lock` | distributed lock protocol |
| `fencing` | fencing tokens for stale-write rejection |
| `gossip` | gossip infection bounds |
| `markov` | Markov chain prediction |
| `merkle` | Merkle proof verification |
| `rwlock` | read-write lock mode transitions, mutual exclusion |
| `per_core_rwlock` | per-core reader aggregation, writer drain, and rwlock mode refinement |
| `vector_clock` | vector clock merge, happens-before, concurrency |

### Nix package manager

| Module | What it verifies |
|---|---|
| `nix_attrset` | attribute set operations (get, has, merge, intersect, remove) |
| `nix_closure` | generic closure computation |
| `nix_list` | list operations (head, tail, elem, concat, gen_list) |
| `nix_string` | string operations (substring, length, replace) |
| `nix_version` | version comparison (same algorithm as Nix's compareVersions) |

### Nickel language semantics

| Module | What it verifies |
|---|---|
| `nickel_priority` | Nickel merge priority total order: Bottom < Neutral < Numeral(n) < Top |
| `nickel_merge_split` | Nickel three-way record split: left-only, center, right-only |
| `nickel_field_merge` | Nickel field-level merge resolution by priority |
| `nickel_record_merge` | Nickel full record merge with identity, disjoint commutativity, and neutral-record associativity proofs |
| `nickel_subtype` | Nickel gradual type subtyping: reflexivity, covariance, record-dict |
| `nickel_polarity` | Nickel contract blame polarity: flip on domain, double-flip identity |
| `nickel_pattern_coverage` | Nickel pattern completeness and overlap detection |

### Networking

| Module | What it verifies |
|---|---|
| `tcp_state` | TCP connection state machine (RFC 793): 11 states, 18 transitions, reachability |

### Database internals

| Module | What it verifies |
|---|---|
| `txn_isolation` | SQL isolation level hierarchy: Read Uncommitted through Serializable |
| `mvcc` | multi-version concurrency control: version visibility, snapshot reads |
| `snapshot_isolation` | write-write conflict detection for snapshot isolation |
| `query_cost` | query plan cost model: sequential scan, index scan, selectivity |
| `index_select` | covering index checks, index selection by estimated cost |
| `txn_dependency` | transaction dependency graph: read-write and write-write conflicts |

### Graph algorithms

| Module | What it verifies |
|---|---|
| `topo_sort` | topological sort via Kahn's algorithm: validity, completeness |
| `cycle_detect` | cycle detection: remaining node count proves acyclicity |
| `reachability` | BFS reachability: soundness, visited-set monotonicity |
| `dominator_tree` | iterative dominator sets, immediate dominators, and dominance frontiers |
| `loop_detection` | back-edge classification, loop bodies over reachability/dominator views, exit edges, and nesting |
| `liveness_analysis` | backward live-in/live-out analysis, fixed-point checks, and live-range projection |
| `ssa_construction` | iterated-dominance-frontier phi placement, SSA renaming, and phi elimination |
| `dead_code_elimination` | dead-instruction classification, block elimination, observable-trace preservation, idempotence |
| `graph_color` | graph 2-coloring (bipartiteness): no adjacent same-color |
| `shortest_path` | unweighted shortest path via BFS: distance bounds |

### Compiler backend

| Module | What it verifies |
|---|---|
| `register_allocation` | live-range overlap graphs, conflict-free register assignment, and deterministic spill fallback |
| `stack_frame_layout` | aligned stack-slot placement, exact rounded frame sizing, and explicit safe-reuse checks over live ranges |
| `wasm_primitives` | exact Wasm value tags, `i32.add` stack discipline, typed local access, linear-memory bounds/grow semantics, control-label branch/return arity, unreachable-stack validation, control-frame stack-height restoration, finite indirect-call table/signature admission, and branch-table target validation |
| `gpu_primitives` | finite GPU program shapes, launch/grid/block/thread bounds, pitched/tensor memory-layout, coalesced access, and shared-tile admission |
| `hdl_ir` / `hdl_emit` / `hdl_adapters` / `hdl_validation` | finite HDL IR admission, deterministic Verilog/SystemVerilog emission, adapter refinement, generated-vector collateral, and tool-evidence classification |

### Scheduling primitives

| Module | What it verifies |
|---|---|
| `deadline_queue` | earliest-deadline-first ordering, deadline comparison |
| `fair_share` | weighted fair allocation: monotonicity, conservation |
| `priority_inversion` | priority inheritance protocol: effective priority, inversion detection |
| `weighted_round_robin` | weighted round-robin: fairness, wrap-around, next consumer |
| `job_dag` | job dependency DAG: all-deps-satisfied, ready-to-run |

### Job runner

| Module | What it verifies |
|---|---|
| `job_state` | job lifecycle FSM: Pending → Ready → Running → Completed/Failed/Cancelled |
| `job_priority` | priority ordering: (class, sequence) total order, higher-first selection |
| `job_slot` | bounded concurrency slot pool: acquire/release, capacity |
| `job_cancel` | cancellation propagation: dependent cancellation, surviving paths |
| `job_idempotency` | idempotency tokens: consume-once, duplicate rejection |
| `durable_history` | append-only durable event history, contiguous prefix, append admission |
| `durable_replay` | replay gate for durable prefix reads and effect deduplication |
| `durable_resume` | checkpoint advancement and crash-recovery resume-point derivation |
| `durable_terminal` | terminal outcome detection and deterministic recovery status |
| `job_retry` | retry decisions: attempts, budget, retryable classification |
| `job_timeout` | deadline computation, timeout checking, not-timed-out-at-start |
| `job_result` | exit code classification: success/failure/error, completion status |

### Serialization

| Module | What it verifies |
|---|---|
| `serialize_inj` | canonical byte encoding injectivity: tag-length-value roundtrips, u64 encode/decode |

### Systems decision logic

| Module | What it verifies |
|---|---|
| `approval` | merge gating: approval counting, stale detection |
| `backoff` | exponential backoff with cap and jitter bounds |
| `batch` | write batch limit checking and flush decisions |
| `bloom` | Bloom filter insert/query monotonicity, no false negatives |
| `bounded_range` | ranges with optional upper bound |
| `cas` | compare-and-swap condition evaluation |
| `chunk` | ceiling-division chunk arithmetic |
| `constant_time_eq` | constant-time 32-byte equality |
| `cost_tracker` | cost accumulation, budget enforcement, model selection |
| `cow` | copy-on-write ref counting, snapshot isolation |
| `drain` | graceful shutdown admission protocol |
| `drain_protocol` | drain with double-check admission |
| `env_scrub` | environment variable scrubbing for sandbox isolation |
| `eviction` | LRU cache eviction with protected entries |
| `eviction_score` | multi-factor eviction scoring |
| `expiration` | TTL calculation, lease/key expiration |
| `hash_bucket` | hash-to-bucket assignment (modular and power-of-two) |
| `kv_version` | KV entry version computation |
| `layout_constraint` | layout constraint resolution |
| `lease` | lease management with disjointness and renewal |
| `monotonic` | advancement predicates for terms, indices |
| `node_lifecycle` | node lifecycle state machine |
| `partition` | non-overlapping gap-free interval partitions |
| `peer_spec` | peer specification parsing |
| `permission` | Unix file permission bit manipulation |
| `pressure` | multi-threshold severity classification |
| `process_state` | process lifecycle state machine |
| `rate_limit` | integer token bucket rate limiting |
| `retry` | retry decision logic with budget tracking |
| `sequential` | sequential access detection, prefetch triggering |
| `signal` | signal set operations |
| `tier` | tier classification for tiered caching |
| `tiered_rate_limit` | two-tier rate limit (global + per-peer) |
| `tiling` | tiling layout: disjoint rects, containment, scroll bounds |
| `trust_level` | trust level resolution and access control |
| `two_phase` | two-phase commit decision logic |
| `type_lattice` | type conversion lattice |
| `typed_state` | compile-time verification pipeline enforcement via phantom types |
| `quota` | resource quota enforcement: remaining capacity, overcommit detection |

### Generic proof infrastructure

| Module | What it verifies |
|---|---|
| `algebraic` | commutative, associative, idempotent, semilattice, partial order predicates |
| `pointwise` | scalar operation lifting to sequences |
| `seq_fold` | recursive counting and folding over sequences |
| `seq_set` | set-theoretic operations over integer sequences |
| `forest` | parent-pointer forest: ancestors, LCA, subtree walks |
| `session_tree` | session tree as forest + seq_set composition |
| `chain` | chain hash integrity with append-preserves proof |
| `choreography_global` | finite global choreography syntax, participants, and well-formedness |
| `choreography_local` | finite local choreography syntax, branch lookup, and merge |
| `choreography_projection` | projectability checks and endpoint projection |
| `choreography_step` | one-step global/local reduction and shared-prefix alignment |
| `debruijn` | de Bruijn shifting, substitution, beta reduction, and scope preservation |
| `acid` | atomicity, consistency, isolation, durability over state vectors |
| `verification_frontier` | finite contract implication, type-bound facts, slice-access certificates, arena provenance, progress budgets, and external-reference admission |

### Cross-module properties

| Module | What it verifies |
|---|---|
| `properties` | 200+ cross-module proofs: signal↔bitwise, permission independence, escrow↔balance, ledger↔transfer, fee conservation, election↔quorum, fencing↔CAS, vector clock↔CRDT, mpmc↔mpsc occupancy, spin↔CAS retry, seqlock↔memory order, RCU↔epoch reclaim, ref count↔hazard pointer, lock ordering↔ticket lock, barrier↔quorum, futex↔notify roundtrip, job state↔timeout↔result, CAS ABA↔pack roundtrip, concurrency protocol↔data structure composition, choreography projection/choice/step correspondence |
| `concurrent_composition` | concurrency protocol ↔ data structure invariant chains across 7 groups |
| `consensus_path` | end-to-end Raft read/write path composition proofs |

### Additional public module coverage

The drift checker treats the compact lists below as README catalog entries for public modules whose detailed descriptions remain in the crate-level `src/lib.rs` documentation. Keep these names synchronized with `pub mod` declarations; add a fuller table row above when a module needs human-facing explanation.

- **beam**: `beam_link`, `beam_mailbox`, `beam_monitor`, `beam_process`, `beam_registry`, `beam_scheduler`, `beam_send`
- **bounded**: `bounded_interval`
- **cache**: `cache_partition`
- **chaoscontrol**: `chaoscontrol_coverage`, `chaoscontrol_fault_schedule`, `chaoscontrol_fault_state`, `chaoscontrol_frontier`, `chaoscontrol_input_tree`, `chaoscontrol_oracle`, `chaoscontrol_scheduler`
- **composite**: `composite_outcome`, `composite_receipt_envelope`
- **config**: `config_admission`, `config_default`, `config_key`, `config_override_precedence`, `config_typed_value`, `config_validation_decision`
- **consistency**: `consistency_history`, `consistency_object`, `consistency_refinement`, `consistency_relations`, `consistency_session`, `consistency_transactional`
- **constant**: `constant_folding`
- **constraint**: `constraint_admission`, `constraint_assignment`, `constraint_consistency`, `constraint_domain`, `constraint_propagation`, `constraint_relation`, `constraint_variable`, `constraint_witness`
- **control**: `control_actuator_saturation`, `control_deadband`, `control_error_signal`, `control_flow_graph`, `control_hysteresis`, `control_rate_limit`, `control_safety_envelope`, `control_setpoint`
- **cron**: `cron_field`, `cron_match`, `cron_next`
- **ct**: `ct_select`
- **data**: `data_validation_admission`, `data_validation_cross_field`, `data_validation_enum_constraint`, `data_validation_error_accumulation`, `data_validation_field_presence`, `data_validation_normalization_decision`, `data_validation_range_constraint`, `data_validation_schema_predicate`
- **ds**: `ds_bounded_array`, `ds_fifo_queue`, `ds_key_value_map`, `ds_priority_queue`, `ds_ring_buffer`, `ds_union_find`
- **false**: `false_sharing`
- **feature**: `feature_flag_gate`, `feature_flag_rollout_bucket`
- **fetch**: `fetch_add`
- **fs**: `fs_path_admission`, `fs_path_containment`, `fs_path_extension_type`, `fs_path_form`, `fs_path_join_split`, `fs_path_normalization`, `fs_path_segment`, `fs_path_traversal`
- **geom**: `geom_admission`, `geom_bounds`, `geom_containment`, `geom_grid_cell`, `geom_intersection`, `geom_orientation`, `geom_point_vector`, `geom_transform`
- **gis**: `gis_bounding_box`, `gis_coordinate_envelope`, `gis_linestring_topology`, `gis_tile_index`
- **graph**: `graph_adjacency`, `graph_component_partition`, `graph_frontier_cut`, `graph_query_composition`, `graph_reachability`, `graph_shortest_path_envelope`, `graph_topological_order`
- **hypervisor**: `hypervisor_dirty_log`, `hypervisor_exit`, `hypervisor_gpa`, `hypervisor_interrupt`, `hypervisor_memslot`, `hypervisor_vcpu`
- **id**: `id_alias_map`, `id_canonicalization`, `id_collision_metadata`, `id_identifier_envelope`, `id_lookup_result`, `id_namespace_scope`, `id_registration_admission`, `id_scoped_uniqueness`
- **index**: `index_query_pagination`
- **instruction**: `instruction_selection`
- **knowledge**: `knowledge_graph`
- **left**: `left_right`
- **metric**: `metric_admission`, `metric_aggregate`, `metric_alert`, `metric_counter`, `metric_gauge`, `metric_histogram`, `metric_id`, `metric_labels`, `metric_window`
- **ml**: `ml_batch_iterator`, `ml_confusion_matrix`, `ml_dataset_split`, `ml_feature_normalization`, `ml_inference_envelope`, `ml_label_encoding`, `ml_tensor_shape`, `ml_top_k`
- **net**: `net_checksum_shape`, `net_endpoint_address`, `net_header_field`, `net_message_sequence`, `net_packet_frame`, `net_parser_admission`, `net_payload_bound`, `net_protocol_admission`
- **nix**: `nix_base32`, `nix_drv_serialize`, `nix_fingerprint`, `nix_hash_truncate`, `nix_store_path`
- **numeric**: `numeric_affine_transform_discrete`, `numeric_checked_arithmetic_envelope`, `numeric_denominator_safe_ratio`, `numeric_fixed_point_scale`, `numeric_histogram_counter`, `numeric_monotone_sequence`, `numeric_prefix_sum`, `numeric_quantized_threshold`
- **opt**: `opt_admission`, `opt_bound`, `opt_candidate_ranking`, `opt_dominance`, `opt_feasible_set`, `opt_frontier`, `opt_objective`, `opt_optimum_witness`
- **parsing**: `parsing_format`
- **physics**: `physics_collision_envelope`, `physics_contact_manifold`, `physics_energy_budget`, `physics_integrator_step`, `physics_momentum_budget`, `physics_time_horizon`, `physics_units`, `physics_vector_bounds`
- **policy**: `policy_action`, `policy_authorization`, `policy_conflict`, `policy_delegation`, `policy_grant_denial`, `policy_principal`, `policy_resource`, `policy_rule`
- **probability**: `probability_statistics`
- **proof**: `proof`, `proof_gap_boundary_audit`
- **raft**: `raft_consensus_core`
- **rect**: `rect_grid_index`
- **resource**: `resource_accounting`
- **scheduling**: `scheduling_calendar`
- **sel4**: `sel4_asid`, `sel4_cap_rights`, `sel4_cdt`, `sel4_cspace`, `sel4_endpoint`, `sel4_fault_ipc`, `sel4_invariants` (kernel invariant plus preservation matrix), `sel4_invocation`, `sel4_irq`, `sel4_notification`, `sel4_ready_queue`, `sel4_reply`, `sel4_tcb`, `sel4_untyped`, `sel4_vspace`
- **serialization**: `serialization_codec`
- **shamir**: `shamir_field`, `shamir_lagrange`, `shamir_poly`, `shamir_reconstruct`, `shamir_split`, `shamir_threshold`
- **signal**: `signal_processing`
- **spreadsheet**: `spreadsheet_address`, `spreadsheet_dependency`, `spreadsheet_formula`, `spreadsheet_recalc`
- **state**: `state_machine_workflow`
- **storage**: `storage_index`
- **surelock**: `surelock_deadlock`, `surelock_id`, `surelock_key`, `surelock_level`, `surelock_lockset`
- **table**: `table_relational`
- **text**: `text_string`
- **type**: `type_unification`
- **ucan**: `ucan_ability`, `ucan_backend_transcript`, `ucan_base64url`, `ucan_capability_merge`, `ucan_caveat`, `ucan_caveat_evaluation`, `ucan_caveat_preservation`, `ucan_chain`, `ucan_cid`, `ucan_claims_presence`, `ucan_compact`, `ucan_delegation_depth`, `ucan_did`, `ucan_header`, `ucan_invocation`, `ucan_json`, `ucan_parsed_capability_shape`, `ucan_parsed_caveat_payload`, `ucan_parsed_caveat_shape`, `ucan_parsed_did_claim`, `ucan_principal_link`, `ucan_proof_reference`, `ucan_proof_reference_collection`, `ucan_proof_reference_list_shape`, `ucan_proof_store_result`, `ucan_proof_traversal`, `ucan_replay_admission`, `ucan_revocation`, `ucan_revocation_check`, `ucan_signature`, `ucan_temporal`, `ucan_temporal_order`, `ucan_token`, `ucan_verification_stage`
- **ungrouped**: `calendar`, `cryptoshedding`, `mesi`, `moesi`, `nonce`, `timeseries`, `zeroize`
- **vision**: `vision_color_layout`, `vision_color_transform_envelope`, `vision_connected_components`, `vision_convolution_window`, `vision_detection_nms`, `vision_image_buffer`, `vision_image_pyramid`, `vision_roi`
- **web**: `web_of_trust_attestation`, `web_of_trust_chain`, `web_of_trust_policy`, `web_of_trust_principal`, `web_of_trust_resolution`, `web_of_trust_revocation`
- **zircon**: `zircon_channel`, `zircon_event`, `zircon_fifo`, `zircon_futex`, `zircon_handle`, `zircon_job`, `zircon_port`, `zircon_rights`, `zircon_socket`, `zircon_timer`, `zircon_wait`

## Usage

```toml
[dependencies]
verified-logic = { git = "https://github.com/OnixResearch/trellis" }
```

```rust
use verified_logic::arithmetic::abs_diff;
use verified_logic::bounds::clamp;
use verified_logic::quorum::majority;
use verified_logic::drain::can_start_op;

let d = abs_diff(10, 3);       // 7
let c = clamp(15, 0, 10);      // 10
let m = majority(5);           // 3
let ok = can_start_op(false);  // true (not draining)
```

For normal `cargo build`, the `vstd` crate erases all ghost code. No
runtime cost from specs, proofs, requires, or ensures. See
[docs/runtime-cost.md](docs/runtime-cost.md) for reproducible normal-build
evidence records.

## Runnable examples

Each runtime example compiles through normal Cargo without invoking Verus. For Verus-focused spec/exec/ensures walkthroughs, see [docs/proof-examples.md](docs/proof-examples.md).

- [`examples/core_basics.rs`](examples/core_basics.rs): `cargo run --example core_basics`
- [`examples/consensus_admission.rs`](examples/consensus_admission.rs): `cargo run --example consensus_admission`
- [`examples/ucan_web_of_trust.rs`](examples/ucan_web_of_trust.rs): `cargo run --example ucan_web_of_trust`
- [`examples/r1cs_single_row.rs`](examples/r1cs_single_row.rs): `cargo run --example r1cs_single_row`
- [`examples/storage_allocator.rs`](examples/storage_allocator.rs): `cargo run --example storage_allocator`
- [`examples/integration_adapters.rs`](examples/integration_adapters.rs): `cargo run --example integration_adapters`

## Development

Start with [CONTRIBUTING.md](CONTRIBUTING.md) for the public workflow, module template, Tracey annotation rules, proof-gap policy, and baseline/final validation expectations. Use [docs/module-guide.md](docs/module-guide.md) to choose modules by task or problem domain, [docs/module-index.md](docs/module-index.md) for objective module maturity facts, [docs/invariant-surface-catalog.md](docs/invariant-surface-catalog.md) for invariant evidence rows, [docs/proof-evidence-bundles.md](docs/proof-evidence-bundles.md) for downstream evidence export, [docs/kani-boundary.md](docs/kani-boundary.md) for complementary Kani evidence boundaries, [docs/proof-artifact-manifest.md](docs/proof-artifact-manifest.md) for exported primitive manifest rows, [docs/ucan-bridge-coverage.md](docs/ucan-bridge-coverage.md) for UCAN bridge coverage, and [docs/cookbook.md](docs/cookbook.md) for short workflow recipes.

```bash
nix develop                # enter devshell
pre-commit install         # install the local Octet pre-commit hook
cargo check                # compile (ghost code erased)
cargo test                 # 4617 unit/property + 13 doc tests
cargo clippy --all-targets -- -D warnings
```

## Verification

```bash
./scripts/verify.rs        # all gates: clippy → tests → verus → coverage
nix build .#checks.$(nix eval --raw --impure --expr 'builtins.currentSystem').octet --no-link -L # Octet lints
./scripts/verus.sh         # verus only
./scripts/check-coverage.rs   # every pub fn has r[impl] annotation
./scripts/check-ensures.rs    # every pub fn in verus! has ensures
./scripts/checker-fixtures.rs # valid/invalid regression fixtures for checker behavior
./scripts/check-verification-policy.rs # Nickel policy typecheck/export freshness
./scripts/check-readme-module-catalog.rs # README module catalog drift and fixture self-test
./scripts/check-crate-metadata.rs # Cargo package metadata discoverability and local path validation
./scripts/check-module-guide.rs # task-oriented module guide link drift and fixture self-test
./scripts/check-module-index.rs # generated module maturity index drift and fixture self-test
./scripts/check-cookbook.rs # cookbook recipe field/link drift and fixture self-test
./scripts/check-examples.rs # runnable example link/run-command drift and fixture self-test
./scripts/check-proof-examples.sh # positive and expected-failure runnable proof examples
cargo run --quiet --bin proof_evidence -- check # deterministic proof-evidence bundle validation
./scripts/check-runtime-evidence.rs # runtime-cost evidence record metadata and fixture self-test
./scripts/check-kani-boundary.rs # Kani external-evidence boundary docs and positive/negative fixtures
./scripts/check-verified-primitive-manifest.rs # exported primitive manifest freshness and positive/negative fixtures
./scripts/check-ucan-bridge-coverage.rs # UCAN bridge coverage manifest and positive/negative fixtures
./scripts/check-readme-metrics.rs # README headline metrics drift; --full refreshes command-backed counts
./scripts/check-proof-gap-inventory.rs # proof-gap counts, exact sites, classifications, and rationale manifest
./scripts/check-upstream-verus-corpus.rs # typed catalog and boundary validation; add --self-test for negative cases
nix build .#checks.$(nix eval --raw --impure --expr 'builtins.currentSystem').upstream-verus-corpus --no-link -L
nix build .#checks.$(nix eval --raw --impure --expr 'builtins.currentSystem').verusfmt-ratchet --no-link -L
nix build .#checks.$(nix eval --raw --impure --expr 'builtins.currentSystem').verus-upgrade-trial --no-link -L
```

`upstream-verus-corpus/catalog.ncl` pins source snapshots for verifier-upgrade
regression selection. Its source-intake check validates exact fixed-output
hashes, license files, and sentinels; it does not run or import upstream proofs.
Promotion to Trellis formal evidence still requires an exact local verifier run,
assumption account, source/proof/tool bindings, and Trellis-owned receipt.

The fail-closed May 17 trial validates Octet Verus `0.2026.05.17.e479cce`:
both complete Trellis runs report `10712 verified, 0 errors`; adapted IronKV
reports `312 verified, 0 errors`; adapted verified-memory-allocator reports
`732 verified, 0 errors`; and unmodified verified-node-replication reports
`254 verified, 0 errors`. The two patches are BLAKE3-bound, preserve assumption
and trusted-marker counts, and are reported only as adapted-source results. The
SOSP microcases remain diagnostic-only and VerusBelt remains a Rocq
semantic-reference audit.

Octet revision `374bd16b26cee2af34211a29bfa531c016811f51` is published and
locked as the exact provider. Trellis's default proof check and wrapper now
consume Octet Verus `0.2026.05.17.e479cce`; the former
`0.2026.03.22.5e66329` package remains only as the receipt's historical
comparison baseline.

The pinned `verusfmt` rail consumes Octet's revision-bound formatter and typed
profile packages; Trellis does not reconstruct a local release fallback. It uses
`upstream-verus-corpus/verusfmt-baseline.txt` as an exact migration ratchet rather
than rewriting the existing proof corpus wholesale. A new nonconforming file, a
stale baseline entry, or central-profile drift fails. The promoted verifier is
admitted by the separate May 17 compatibility receipt; package identity and
baseline membership alone still do not claim formatting or proof success.

The Octet check uses `octet-baseline.json` to accept current warning-level
findings while rejecting any new unbaselined stable finding ID. The local
pre-commit hook delegates to that same pinned Nix check.

## Structure

```
src/lib.rs              ← crate root, 581 public module declarations
src/<module>.rs         ← spec fn + exec fn + proof fn + #[cfg(test)] tests
docs/requirements.md    ← canonical requirement prose and Tracey anchors
scripts/verus.sh        ← Octet production-Verus package resolver
scripts/verify.rs       ← full verification pipeline
scripts/check-*.rs      ← annotation enforcement
```

Most modules follow the same pattern (tracked exceptions are listed in
`openspec/specs/verification-gaps/spec.md`):

1. `pub open spec fn *_spec(...)` — mathematical definition with `r[depends]` annotation
2. `pub fn *(...)` with `ensures result == *_spec(args)` — runtime code with `r[impl]` annotation
3. `proof fn` — properties of the spec with `r[verify]` annotation
4. `#[cfg(test)]` — unit tests and proptest property tests

## Trusted and proof-gap items

`16` items are marked `#[verifier::external_body]` across `6` files. The
previous `src/instruction_selection.rs` runtime-only public API exemptions now
have direct Tracey implementation and test coverage.

**16 permanent** (architectural trust boundaries):

- **4 legacy cryptographic hash axioms / exec bridges** (`chain.rs`,
  `snapshot_integrity.rs`) — BLAKE3-style hashing is modeled with
  uninterpreted specs plus trusted collision-resistance/runtime wrappers;
  equal-input determinism now follows directly from pure spec equality.
- **1 version comparison bridge** (`nix_version.rs`) — the recursive `cv_fuel`
  proof lemmas are now internalized, while the exec/spec connection still
  exceeds solver budget.
- **10 UCAN codec/signature bridges** (`ucan_did.rs`, `ucan_signature.rs`) —
  base58btc and Ed25519 behavior is modeled behind external codec/crypto
  contracts; `ucan_cid.rs` now composes its CID multibase round-trip model from
  the shared DID base58btc contract instead of carrying a separate CID axiom,
  and public DID decode now composes verified prefix, base58, varint, and
  key-suffix primitives around the remaining base58btc contract. Base58btc
  decode also performs verified alphabet screening before the trusted codec
  core. UCAN signature byte-level entrypoints now reject malformed key/signature
  lengths through verified constructors before invoking trusted Ed25519 crypto.
- **1 web-of-trust crypto bridge** (`web_of_trust_principal.rs`) — principal
  digest derivation stays behind a trusted runtime/model boundary; attestation
  and revocation signature-input construction and comparison are now verified
  from canonical bytes.

**0 closeable** — the former runtime-only public APIs in
`src/instruction_selection.rs` now have direct Tracey implementation and test
coverage for their well-formedness, matching, tiling, coverage, soundness,
projection, evaluation, and validity contracts.
Def-site collection, IDF computation, SSA-property checking, and phi elimination
are internalized now; runtime tests and manual oracles still cover the stronger
remaining SSA semantic claims.

In addition to `external_body`, the current audit also tracks:

- `0` `assume_specification[...]` exec bridges
- `0` runtime-only `@verified-logic:exempt` public API proof gaps
- `0` `@verified-logic:no-ensures` markers
- `0` `assume(...)` soundness oracles
- `0` placeholder-spec surfaces

All trusted/proof-gap items remain listed in `openspec/specs/verification-gaps/spec.md`, and `docs/proof-gap-sites.tsv` is the checked per-site source for exact trusted-boundary rationale.

## License

Trellis and the `verified-logic` crate are licensed under `MPL-2.0`; see [LICENSE](LICENSE). Modifications to covered files remain available under MPL terms, while a larger work may keep separate files under other licenses subject to the MPL.

Earlier grants remain valid, and third-party references retain their own terms. Machine-checked properties are bounded to the admitted source, verifier, assumptions, and receipts; MPL metadata does not certify downstream behavior or transfer whole-system proof authority.

## References

- Lattice/Salt verification-product patterns — external design reference only; not vendored, executed, or treated as trusted Trellis validation evidence.
- [Kani: A Model Checker for Rust](https://arxiv.org/html/2607.01504v1) — external Kani model-checking reference for complementary runtime evidence only; not treated as a Trellis proof artifact.
- [verus-lang/verus-belt](https://github.com/verus-lang/verus-belt) — semantic-foundation and assumption-audit corpus tied to its recorded Verus snapshot; not a soundness certificate for later releases.
- [verus-lang/paper-sosp24-artifact](https://github.com/verus-lang/paper-sosp24-artifact) — pinned reproducibility and verifier-upgrade benchmark corpus with mixed upstream license boundaries.
- [verus-lang/verified-memory-allocator](https://github.com/verus-lang/verified-memory-allocator) — allocator/raw-pointer proof-pattern corpus only; its feature omissions preclude production allocator adoption here.
- [verus-lang/verified-ironkv](https://github.com/verus-lang/verified-ironkv) — host-layer proof and distributed-boundary regression corpus; no crash-recovery or whole-distributed-system claim transfers.
- [verus-lang/verified-node-replication](https://github.com/verus-lang/verified-node-replication) — local multicore/NUMA proof corpus only; May 17 compatibility does not turn it into network replication, consensus, or an admitted runtime dependency.
- [verus-lang/verusfmt](https://github.com/verus-lang/verusfmt) — pinned source-format ratchet; formatter output is not proof evidence.
- [Steel](https://github.com/mattwparas/steel) — external Scheme interpreter reference for finite reader, interning, module, contract, bytecode, and value-graph primitive ideas only; not vendored, executed, or treated as trusted Trellis validation evidence.
- [`stonysystems/jetpack` at `c03e318ec355b11edd42aac56c68d0765f88d1d2`](https://github.com/stonysystems/jetpack/tree/c03e318ec355b11edd42aac56c68d0765f88d1d2) and the [OSDI 2026 paper](https://www.usenix.org/system/files/osdi26-tang.pdf) — pinned MIT-licensed design, TLA+, and fault-scenario references for reusable crash-fault fast-path proof obligations; external proofs, implementation behavior, and benchmark claims do not transfer to Trellis.
