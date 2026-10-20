//! Bounded graph-query composition predicates.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GraphQueryModel {
    pub nodes_declared: bool,
    pub labels_declared: bool,
    pub triples_admitted: bool,
    pub variables_declared: bool,
    pub literals_admitted: bool,
    pub within_query_bounds: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TriplePatternMatch {
    pub subject_ok: bool,
    pub predicate_ok: bool,
    pub object_ok: bool,
    pub repeated_vars_consistent: bool,
    pub binding_in_model: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BindingExtension {
    pub preserves_existing: bool,
    pub added_pairs_admitted: bool,
    pub no_conflicts: bool,
    pub variable_count: usize,
    pub max_variables: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PathWitness {
    pub endpoints_match: bool,
    pub every_step_has_triple: bool,
    pub labels_match: bool,
    pub length: usize,
    pub max_length: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JoinFilterWitness {
    pub left_row_admitted: bool,
    pub right_row_admitted: bool,
    pub bindings_compatible: bool,
    pub filter_true: bool,
    pub output_row_count: usize,
    pub max_rows: usize,
    pub source_witnesses_preserved: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IndexScanEquivalence {
    pub index_hits_in_scan: bool,
    pub scan_hits_in_index: bool,
    pub same_query_bounds: bool,
    pub lookup_result_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SerializedGraphQueryEnvelope {
    pub schema_supported: bool,
    pub header_admitted: bool,
    pub payload_admitted: bool,
    pub trailer_admitted: bool,
    pub result_window_bounded: bool,
    pub model_references_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GraphQueryResultFacade {
    pub model_ok: bool,
    pub pattern_ok: bool,
    pub binding_ok: bool,
    pub path_ok: bool,
    pub join_filter_ok: bool,
    pub index_equiv_ok: bool,
    pub serialized_envelope_ok: bool,
}

// r[depends composite-primitives.graph-query.model]
pub open spec fn graph_query_model_spec(e: GraphQueryModel) -> bool {
    e.nodes_declared && e.labels_declared && e.triples_admitted && e.variables_declared
        && e.literals_admitted && e.within_query_bounds
}

// r[impl composite-primitives.graph-query.model]
pub fn graph_query_model_valid(e: GraphQueryModel) -> (result: bool)
    ensures result == graph_query_model_spec(e)
{
    e.nodes_declared && e.labels_declared && e.triples_admitted && e.variables_declared
        && e.literals_admitted && e.within_query_bounds
}

// r[verify composite-primitives.graph-query.model]
pub proof fn valid_graph_query_model_has_declared_references(e: GraphQueryModel)
    requires graph_query_model_spec(e)
    ensures e.nodes_declared, e.labels_declared, e.triples_admitted, e.within_query_bounds
{}

// r[depends composite-primitives.graph-query.triple-pattern]
pub open spec fn triple_pattern_match_spec(e: TriplePatternMatch) -> bool {
    e.subject_ok && e.predicate_ok && e.object_ok && e.repeated_vars_consistent && e.binding_in_model
}

// r[impl composite-primitives.graph-query.triple-pattern]
pub fn triple_pattern_match_valid(e: TriplePatternMatch) -> (result: bool)
    ensures result == triple_pattern_match_spec(e)
{
    e.subject_ok && e.predicate_ok && e.object_ok && e.repeated_vars_consistent && e.binding_in_model
}

// r[verify composite-primitives.graph-query.triple-pattern]
pub proof fn valid_triple_pattern_has_consistent_bindings(e: TriplePatternMatch)
    requires triple_pattern_match_spec(e)
    ensures e.repeated_vars_consistent, e.binding_in_model
{}

// r[depends composite-primitives.graph-query.binding-extension]
pub open spec fn binding_extension_spec(e: BindingExtension) -> bool {
    e.preserves_existing && e.added_pairs_admitted && e.no_conflicts && e.variable_count <= e.max_variables
}

// r[impl composite-primitives.graph-query.binding-extension]
pub fn binding_extension_valid(e: BindingExtension) -> (result: bool)
    ensures result == binding_extension_spec(e)
{
    e.preserves_existing && e.added_pairs_admitted && e.no_conflicts && e.variable_count <= e.max_variables
}

// r[verify composite-primitives.graph-query.binding-extension]
pub proof fn valid_binding_extension_has_no_conflicts(e: BindingExtension)
    requires binding_extension_spec(e)
    ensures e.preserves_existing, e.no_conflicts, e.variable_count <= e.max_variables
{}

// r[depends composite-primitives.graph-query.path-witness]
pub open spec fn path_witness_spec(e: PathWitness) -> bool {
    e.endpoints_match && e.every_step_has_triple && e.labels_match && e.length <= e.max_length
}

// r[impl composite-primitives.graph-query.path-witness]
pub fn path_witness_valid(e: PathWitness) -> (result: bool)
    ensures result == path_witness_spec(e)
{
    e.endpoints_match && e.every_step_has_triple && e.labels_match && e.length <= e.max_length
}

// r[verify composite-primitives.graph-query.path-witness]
pub proof fn valid_path_witness_has_backed_steps(e: PathWitness)
    requires path_witness_spec(e)
    ensures e.endpoints_match, e.every_step_has_triple, e.length <= e.max_length
{}

// r[depends composite-primitives.graph-query.join-filter]
pub open spec fn join_filter_witness_spec(e: JoinFilterWitness) -> bool {
    e.left_row_admitted && e.right_row_admitted && e.bindings_compatible && e.filter_true
        && e.output_row_count <= e.max_rows && e.source_witnesses_preserved
}

// r[impl composite-primitives.graph-query.join-filter]
pub fn join_filter_witness_valid(e: JoinFilterWitness) -> (result: bool)
    ensures result == join_filter_witness_spec(e)
{
    e.left_row_admitted && e.right_row_admitted && e.bindings_compatible && e.filter_true
        && e.output_row_count <= e.max_rows && e.source_witnesses_preserved
}

// r[verify composite-primitives.graph-query.join-filter]
pub proof fn valid_join_filter_preserves_source_witnesses(e: JoinFilterWitness)
    requires join_filter_witness_spec(e)
    ensures e.bindings_compatible, e.filter_true, e.source_witnesses_preserved
{}

// r[depends composite-primitives.graph-query.index-equivalence]
pub open spec fn index_scan_equivalence_spec(e: IndexScanEquivalence) -> bool {
    e.index_hits_in_scan && e.scan_hits_in_index && e.same_query_bounds && e.lookup_result_admitted
}

// r[impl composite-primitives.graph-query.index-equivalence]
pub fn index_scan_equivalence_valid(e: IndexScanEquivalence) -> (result: bool)
    ensures result == index_scan_equivalence_spec(e)
{
    e.index_hits_in_scan && e.scan_hits_in_index && e.same_query_bounds && e.lookup_result_admitted
}

// r[verify composite-primitives.graph-query.index-equivalence]
pub proof fn valid_index_scan_equivalence_is_bidirectional(e: IndexScanEquivalence)
    requires index_scan_equivalence_spec(e)
    ensures e.index_hits_in_scan, e.scan_hits_in_index, e.same_query_bounds
{}

// r[depends composite-primitives.graph-query.serialized-envelope]
pub open spec fn serialized_graph_query_envelope_spec(e: SerializedGraphQueryEnvelope) -> bool {
    e.schema_supported && e.header_admitted && e.payload_admitted && e.trailer_admitted
        && e.result_window_bounded && e.model_references_admitted
}

// r[impl composite-primitives.graph-query.serialized-envelope]
pub fn serialized_graph_query_envelope_valid(e: SerializedGraphQueryEnvelope) -> (result: bool)
    ensures result == serialized_graph_query_envelope_spec(e)
{
    e.schema_supported && e.header_admitted && e.payload_admitted && e.trailer_admitted
        && e.result_window_bounded && e.model_references_admitted
}

// r[verify composite-primitives.graph-query.serialized-envelope]
pub proof fn valid_serialized_graph_query_envelope_has_supported_schema(e: SerializedGraphQueryEnvelope)
    requires serialized_graph_query_envelope_spec(e)
    ensures e.schema_supported, e.result_window_bounded, e.model_references_admitted
{}

// r[depends composite-primitives.graph-query.result-soundness]
pub open spec fn graph_query_result_facade_spec(e: GraphQueryResultFacade) -> bool {
    e.model_ok && e.pattern_ok && e.binding_ok && e.path_ok && e.join_filter_ok
        && e.index_equiv_ok && e.serialized_envelope_ok
}

// r[impl composite-primitives.graph-query.result-soundness]
pub fn graph_query_result_facade_valid(e: GraphQueryResultFacade) -> (result: bool)
    ensures result == graph_query_result_facade_spec(e)
{
    e.model_ok && e.pattern_ok && e.binding_ok && e.path_ok && e.join_filter_ok
        && e.index_equiv_ok && e.serialized_envelope_ok
}

// r[verify composite-primitives.graph-query.result-soundness]
pub proof fn valid_graph_query_result_facade_carries_all_proofs(e: GraphQueryResultFacade)
    requires graph_query_result_facade_spec(e)
    ensures e.model_ok, e.pattern_ok, e.binding_ok, e.path_ok, e.join_filter_ok, e.index_equiv_ok, e.serialized_envelope_ok
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // r[verify composite-primitives.graph-query.model]
    fn accepts_complete_model() {
        let e = GraphQueryModel {
            nodes_declared: true,
            labels_declared: true,
            triples_admitted: true,
            variables_declared: true,
            literals_admitted: true,
            within_query_bounds: true,
        };
        assert!(graph_query_model_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.triple-pattern]
    fn rejects_inconsistent_repeated_variable() {
        let e = TriplePatternMatch {
            subject_ok: true,
            predicate_ok: true,
            object_ok: true,
            repeated_vars_consistent: false,
            binding_in_model: true,
        };
        assert!(!triple_pattern_match_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.binding-extension]
    fn rejects_conflicting_binding_extension() {
        let e = BindingExtension {
            preserves_existing: true,
            added_pairs_admitted: true,
            no_conflicts: false,
            variable_count: 2,
            max_variables: 4,
        };
        assert!(!binding_extension_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.path-witness]
    fn rejects_broken_path_step() {
        let e = PathWitness {
            endpoints_match: true,
            every_step_has_triple: false,
            labels_match: true,
            length: 2,
            max_length: 4,
        };
        assert!(!path_witness_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.join-filter]
    fn accepts_compatible_join_filter_witness() {
        let e = JoinFilterWitness {
            left_row_admitted: true,
            right_row_admitted: true,
            bindings_compatible: true,
            filter_true: true,
            output_row_count: 1,
            max_rows: 8,
            source_witnesses_preserved: true,
        };
        assert!(join_filter_witness_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.index-equivalence]
    fn rejects_missing_index_scan_hit() {
        let e = IndexScanEquivalence {
            index_hits_in_scan: true,
            scan_hits_in_index: false,
            same_query_bounds: true,
            lookup_result_admitted: true,
        };
        assert!(!index_scan_equivalence_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.serialized-envelope]
    fn rejects_unsupported_serialized_schema() {
        let e = SerializedGraphQueryEnvelope {
            schema_supported: false,
            header_admitted: true,
            payload_admitted: true,
            trailer_admitted: true,
            result_window_bounded: true,
            model_references_admitted: true,
        };
        assert!(!serialized_graph_query_envelope_valid(e));
    }

    #[test]
    // r[verify composite-primitives.graph-query.result-soundness]
    fn accepts_full_result_facade() {
        let e = GraphQueryResultFacade {
            model_ok: true,
            pattern_ok: true,
            binding_ok: true,
            path_ok: true,
            join_filter_ok: true,
            index_equiv_ok: true,
            serialized_envelope_ok: true,
        };
        assert!(graph_query_result_facade_valid(e));
    }
}
