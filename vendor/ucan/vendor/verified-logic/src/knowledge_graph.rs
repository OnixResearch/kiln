//! Exact finite knowledge-graph primitive envelopes.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgTermMetadata {
    pub subject_declared: bool,
    pub predicate_declared: bool,
    pub object_declared: bool,
    pub literal_or_object: bool,
    pub namespace_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgTripleEnvelope {
    pub subject_admitted: bool,
    pub predicate_admitted: bool,
    pub object_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgPropertyEnvelope {
    pub node_label_declared: bool,
    pub edge_label_declared: bool,
    pub property_key_declared: bool,
    pub property_value_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgOntologyEnvelope {
    pub class_admitted: bool,
    pub relation_admitted: bool,
    pub domain_ok: bool,
    pub range_ok: bool,
    pub cardinality_ok: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgProvenanceEnvelope {
    pub source_present: bool,
    pub method_admitted: bool,
    pub confidence_admitted: bool,
    pub version_admitted: bool,
    pub derivation_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgAlignmentEnvelope {
    pub alias_admitted: bool,
    pub witness_present: bool,
    pub canonical_admitted: bool,
    pub conflict: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgQueryShapeEnvelope {
    pub start_admitted: bool,
    pub predicate_filter_admitted: bool,
    pub bounded_path: bool,
    pub variables_bounded: bool,
    pub projection_admitted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgInferenceEnvelope {
    pub rule_admitted: bool,
    pub input_facts_admitted: bool,
    pub derived_fact_admitted: bool,
    pub within_closure_bound: bool,
    pub witness_present: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgEmbeddingIndexEnvelope {
    pub embedding_id_admitted: bool,
    pub dimension_matches: bool,
    pub indexed_item_admitted: bool,
    pub metric_admitted: bool,
    pub version_admitted: bool,
    pub quality_claim: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KgAdmissionEnvelope {
    pub term_ok: bool,
    pub triple_ok: bool,
    pub property_ok: bool,
    pub ontology_ok: bool,
    pub provenance_ok: bool,
    pub alignment_ok: bool,
    pub query_shape_ok: bool,
    pub inference_ok: bool,
    pub embedding_index_ok: bool,
    pub external_claim: bool,
}

// r[depends domain-primitives.knowledge-graph.term]
pub open spec fn kg_term_admitted_spec(x: KgTermMetadata) -> bool {
    x.subject_declared && x.predicate_declared && x.object_declared && x.literal_or_object && x.namespace_admitted
}

// r[impl domain-primitives.knowledge-graph.term]
pub fn kg_term_admitted(x: KgTermMetadata) -> (result: bool)
    ensures result == kg_term_admitted_spec(x)
{
    x.subject_declared && x.predicate_declared && x.object_declared && x.literal_or_object && x.namespace_admitted
}

// r[verify domain-primitives.knowledge-graph.term]
pub proof fn admitted_kg_term_has_declared_predicate(x: KgTermMetadata)
    requires kg_term_admitted_spec(x)
    ensures x.predicate_declared, x.namespace_admitted
{}

// r[depends domain-primitives.knowledge-graph.triple]
pub open spec fn kg_triple_admitted_spec(x: KgTripleEnvelope) -> bool {
    x.subject_admitted && x.predicate_admitted && x.object_admitted
}

// r[impl domain-primitives.knowledge-graph.triple]
pub fn kg_triple_admitted(x: KgTripleEnvelope) -> (result: bool)
    ensures result == kg_triple_admitted_spec(x)
{
    x.subject_admitted && x.predicate_admitted && x.object_admitted
}

// r[verify domain-primitives.knowledge-graph.triple]
pub proof fn admitted_kg_triple_has_endpoints(x: KgTripleEnvelope)
    requires kg_triple_admitted_spec(x)
    ensures x.subject_admitted, x.predicate_admitted, x.object_admitted
{}

// r[depends domain-primitives.knowledge-graph.property]
pub open spec fn kg_property_admitted_spec(x: KgPropertyEnvelope) -> bool {
    x.node_label_declared && x.edge_label_declared && x.property_key_declared && x.property_value_admitted
}

// r[impl domain-primitives.knowledge-graph.property]
pub fn kg_property_admitted(x: KgPropertyEnvelope) -> (result: bool)
    ensures result == kg_property_admitted_spec(x)
{
    x.node_label_declared && x.edge_label_declared && x.property_key_declared && x.property_value_admitted
}

// r[verify domain-primitives.knowledge-graph.property]
pub proof fn admitted_kg_property_has_declared_key(x: KgPropertyEnvelope)
    requires kg_property_admitted_spec(x)
    ensures x.property_key_declared, x.property_value_admitted
{}

// r[depends domain-primitives.knowledge-graph.ontology]
pub open spec fn kg_ontology_admitted_spec(x: KgOntologyEnvelope) -> bool {
    x.class_admitted && x.relation_admitted && x.domain_ok && x.range_ok && x.cardinality_ok
}

// r[impl domain-primitives.knowledge-graph.ontology]
pub fn kg_ontology_admitted(x: KgOntologyEnvelope) -> (result: bool)
    ensures result == kg_ontology_admitted_spec(x)
{
    x.class_admitted && x.relation_admitted && x.domain_ok && x.range_ok && x.cardinality_ok
}

// r[verify domain-primitives.knowledge-graph.ontology]
pub proof fn admitted_kg_ontology_has_range(x: KgOntologyEnvelope)
    requires kg_ontology_admitted_spec(x)
    ensures x.range_ok, x.domain_ok
{}

// r[depends domain-primitives.knowledge-graph.provenance]
pub open spec fn kg_provenance_admitted_spec(x: KgProvenanceEnvelope) -> bool {
    x.source_present && x.method_admitted && x.confidence_admitted && x.version_admitted && x.derivation_admitted
}

// r[impl domain-primitives.knowledge-graph.provenance]
pub fn kg_provenance_admitted(x: KgProvenanceEnvelope) -> (result: bool)
    ensures result == kg_provenance_admitted_spec(x)
{
    x.source_present && x.method_admitted && x.confidence_admitted && x.version_admitted && x.derivation_admitted
}

// r[verify domain-primitives.knowledge-graph.provenance]
pub proof fn admitted_kg_provenance_has_source(x: KgProvenanceEnvelope)
    requires kg_provenance_admitted_spec(x)
    ensures x.source_present, x.derivation_admitted
{}

// r[depends domain-primitives.knowledge-graph.alignment]
pub open spec fn kg_alignment_admitted_spec(x: KgAlignmentEnvelope) -> bool {
    x.alias_admitted && x.witness_present && x.canonical_admitted && !x.conflict
}

// r[impl domain-primitives.knowledge-graph.alignment]
pub fn kg_alignment_admitted(x: KgAlignmentEnvelope) -> (result: bool)
    ensures result == kg_alignment_admitted_spec(x)
{
    x.alias_admitted && x.witness_present && x.canonical_admitted && !x.conflict
}

// r[verify domain-primitives.knowledge-graph.alignment]
pub proof fn admitted_kg_alignment_has_no_conflict(x: KgAlignmentEnvelope)
    requires kg_alignment_admitted_spec(x)
    ensures x.witness_present, !x.conflict
{}

// r[depends domain-primitives.knowledge-graph.query-shape]
pub open spec fn kg_query_shape_admitted_spec(x: KgQueryShapeEnvelope) -> bool {
    x.start_admitted && x.predicate_filter_admitted && x.bounded_path && x.variables_bounded && x.projection_admitted
}

// r[impl domain-primitives.knowledge-graph.query-shape]
pub fn kg_query_shape_admitted(x: KgQueryShapeEnvelope) -> (result: bool)
    ensures result == kg_query_shape_admitted_spec(x)
{
    x.start_admitted && x.predicate_filter_admitted && x.bounded_path && x.variables_bounded && x.projection_admitted
}

// r[verify domain-primitives.knowledge-graph.query-shape]
pub proof fn admitted_kg_query_shape_is_bounded(x: KgQueryShapeEnvelope)
    requires kg_query_shape_admitted_spec(x)
    ensures x.bounded_path, x.variables_bounded
{}

// r[depends domain-primitives.knowledge-graph.inference]
pub open spec fn kg_inference_admitted_spec(x: KgInferenceEnvelope) -> bool {
    x.rule_admitted && x.input_facts_admitted && x.derived_fact_admitted && x.within_closure_bound && x.witness_present
}

// r[impl domain-primitives.knowledge-graph.inference]
pub fn kg_inference_admitted(x: KgInferenceEnvelope) -> (result: bool)
    ensures result == kg_inference_admitted_spec(x)
{
    x.rule_admitted && x.input_facts_admitted && x.derived_fact_admitted && x.within_closure_bound && x.witness_present
}

// r[verify domain-primitives.knowledge-graph.inference]
pub proof fn admitted_kg_inference_has_witness_and_bound(x: KgInferenceEnvelope)
    requires kg_inference_admitted_spec(x)
    ensures x.witness_present, x.within_closure_bound
{}

// r[depends domain-primitives.knowledge-graph.embedding-index]
pub open spec fn kg_embedding_index_admitted_spec(x: KgEmbeddingIndexEnvelope) -> bool {
    x.embedding_id_admitted && x.dimension_matches && x.indexed_item_admitted && x.metric_admitted && x.version_admitted && !x.quality_claim
}

// r[impl domain-primitives.knowledge-graph.embedding-index]
pub fn kg_embedding_index_admitted(x: KgEmbeddingIndexEnvelope) -> (result: bool)
    ensures result == kg_embedding_index_admitted_spec(x)
{
    x.embedding_id_admitted && x.dimension_matches && x.indexed_item_admitted && x.metric_admitted && x.version_admitted && !x.quality_claim
}

// r[verify domain-primitives.knowledge-graph.embedding-index]
pub proof fn admitted_kg_embedding_index_has_dimension_without_quality_claim(x: KgEmbeddingIndexEnvelope)
    requires kg_embedding_index_admitted_spec(x)
    ensures x.dimension_matches, !x.quality_claim
{}

// r[depends domain-primitives.knowledge-graph.admission]
pub open spec fn kg_admission_admitted_spec(x: KgAdmissionEnvelope) -> bool {
    x.term_ok && x.triple_ok && x.property_ok && x.ontology_ok && x.provenance_ok && x.alignment_ok && x.query_shape_ok && x.inference_ok && x.embedding_index_ok && !x.external_claim
}

// r[impl domain-primitives.knowledge-graph.admission]
pub fn kg_admission_admitted(x: KgAdmissionEnvelope) -> (result: bool)
    ensures result == kg_admission_admitted_spec(x)
{
    x.term_ok && x.triple_ok && x.property_ok && x.ontology_ok && x.provenance_ok && x.alignment_ok && x.query_shape_ok && x.inference_ok && x.embedding_index_ok && !x.external_claim
}

// r[verify domain-primitives.knowledge-graph.admission]
pub proof fn admitted_kg_admission_has_all_finite_checks(x: KgAdmissionEnvelope)
    requires kg_admission_admitted_spec(x)
    ensures
        x.term_ok,
        x.triple_ok,
        x.property_ok,
        x.ontology_ok,
        x.provenance_ok,
        x.alignment_ok,
        x.query_shape_ok,
        x.inference_ok,
        x.embedding_index_ok,
        !x.external_claim
{}
} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knowledge_graph_terms_accept_and_reject() {
        assert!(kg_term_admitted(KgTermMetadata {
            subject_declared: true,
            predicate_declared: true,
            object_declared: true,
            literal_or_object: true,
            namespace_admitted: true
        }));
        assert!(!kg_term_admitted(KgTermMetadata {
            subject_declared: true,
            predicate_declared: false,
            object_declared: true,
            literal_or_object: true,
            namespace_admitted: true
        }));
    }

    #[test]
    fn knowledge_graph_triples_accept_and_reject() {
        assert!(kg_triple_admitted(KgTripleEnvelope {
            subject_admitted: true,
            predicate_admitted: true,
            object_admitted: true
        }));
        assert!(!kg_triple_admitted(KgTripleEnvelope {
            subject_admitted: false,
            predicate_admitted: true,
            object_admitted: true
        }));
    }

    #[test]
    fn knowledge_graph_property_accepts_declared_keys_only() {
        assert!(kg_property_admitted(KgPropertyEnvelope {
            node_label_declared: true,
            edge_label_declared: true,
            property_key_declared: true,
            property_value_admitted: true
        }));
        assert!(!kg_property_admitted(KgPropertyEnvelope {
            node_label_declared: true,
            edge_label_declared: true,
            property_key_declared: false,
            property_value_admitted: true
        }));
    }

    #[test]
    fn knowledge_graph_ontology_rejects_range_violation() {
        assert!(kg_ontology_admitted(KgOntologyEnvelope {
            class_admitted: true,
            relation_admitted: true,
            domain_ok: true,
            range_ok: true,
            cardinality_ok: true
        }));
        assert!(!kg_ontology_admitted(KgOntologyEnvelope {
            class_admitted: true,
            relation_admitted: true,
            domain_ok: true,
            range_ok: false,
            cardinality_ok: true
        }));
    }

    #[test]
    fn knowledge_graph_provenance_requires_source() {
        assert!(kg_provenance_admitted(KgProvenanceEnvelope {
            source_present: true,
            method_admitted: true,
            confidence_admitted: true,
            version_admitted: true,
            derivation_admitted: true
        }));
        assert!(!kg_provenance_admitted(KgProvenanceEnvelope {
            source_present: false,
            method_admitted: true,
            confidence_admitted: true,
            version_admitted: true,
            derivation_admitted: true
        }));
    }

    #[test]
    fn knowledge_graph_alignment_rejects_conflict() {
        assert!(kg_alignment_admitted(KgAlignmentEnvelope {
            alias_admitted: true,
            witness_present: true,
            canonical_admitted: true,
            conflict: false
        }));
        assert!(!kg_alignment_admitted(KgAlignmentEnvelope {
            alias_admitted: true,
            witness_present: true,
            canonical_admitted: true,
            conflict: true
        }));
    }

    #[test]
    fn knowledge_graph_query_shape_requires_bounds() {
        assert!(kg_query_shape_admitted(KgQueryShapeEnvelope {
            start_admitted: true,
            predicate_filter_admitted: true,
            bounded_path: true,
            variables_bounded: true,
            projection_admitted: true
        }));
        assert!(!kg_query_shape_admitted(KgQueryShapeEnvelope {
            start_admitted: true,
            predicate_filter_admitted: true,
            bounded_path: false,
            variables_bounded: true,
            projection_admitted: true
        }));
    }

    #[test]
    fn knowledge_graph_inference_requires_witness_and_bound() {
        assert!(kg_inference_admitted(KgInferenceEnvelope {
            rule_admitted: true,
            input_facts_admitted: true,
            derived_fact_admitted: true,
            within_closure_bound: true,
            witness_present: true
        }));
        assert!(!kg_inference_admitted(KgInferenceEnvelope {
            rule_admitted: true,
            input_facts_admitted: true,
            derived_fact_admitted: true,
            within_closure_bound: false,
            witness_present: true
        }));
    }

    #[test]
    fn knowledge_graph_embedding_index_rejects_quality_claims() {
        assert!(kg_embedding_index_admitted(KgEmbeddingIndexEnvelope {
            embedding_id_admitted: true,
            dimension_matches: true,
            indexed_item_admitted: true,
            metric_admitted: true,
            version_admitted: true,
            quality_claim: false
        }));
        assert!(!kg_embedding_index_admitted(KgEmbeddingIndexEnvelope {
            embedding_id_admitted: true,
            dimension_matches: false,
            indexed_item_admitted: true,
            metric_admitted: true,
            version_admitted: true,
            quality_claim: false
        }));
        assert!(!kg_embedding_index_admitted(KgEmbeddingIndexEnvelope {
            embedding_id_admitted: true,
            dimension_matches: true,
            indexed_item_admitted: true,
            metric_admitted: true,
            version_admitted: true,
            quality_claim: true
        }));
    }

    #[test]
    fn knowledge_graph_admission_composes_all_checks() {
        assert!(kg_admission_admitted(KgAdmissionEnvelope {
            term_ok: true,
            triple_ok: true,
            property_ok: true,
            ontology_ok: true,
            provenance_ok: true,
            alignment_ok: true,
            query_shape_ok: true,
            inference_ok: true,
            embedding_index_ok: true,
            external_claim: false
        }));
        assert!(!kg_admission_admitted(KgAdmissionEnvelope {
            term_ok: true,
            triple_ok: true,
            property_ok: true,
            ontology_ok: true,
            provenance_ok: true,
            alignment_ok: true,
            query_shape_ok: true,
            inference_ok: false,
            embedding_index_ok: true,
            external_claim: false
        }));
        assert!(!kg_admission_admitted(KgAdmissionEnvelope {
            term_ok: true,
            triple_ok: true,
            property_ok: true,
            ontology_ok: true,
            provenance_ok: true,
            alignment_ok: true,
            query_shape_ok: true,
            inference_ok: true,
            embedding_index_ok: true,
            external_claim: true
        }));
    }
}
