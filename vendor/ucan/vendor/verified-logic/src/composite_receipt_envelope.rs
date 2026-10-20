//! Bounded composite receipt and evidence-envelope semantics.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReceiptHeaderEnvelope {
    pub schema: crate::serialization_codec::SchemaVersionEnvelope,
    pub section_count: usize,
    pub max_sections: usize,
    pub artifact_count: usize,
    pub max_artifacts: usize,
    pub evidence_count: usize,
    pub max_evidence: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArtifactReferenceEnvelope {
    pub artifact_index: usize,
    pub artifact_count: usize,
    pub bytes: crate::serialization_codec::ByteRange,
    pub buffer_len: usize,
    pub segment: crate::storage_index::SegmentMap,
    pub declared: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ValidationWindowEnvelope {
    pub start_tick: u64,
    pub end_tick: u64,
    pub result_tick: u64,
    pub max_duration: u64,
    pub cursor: crate::storage_index::CursorBounds,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReceiptEvidenceEnvelope {
    pub artifact: ArtifactReferenceEnvelope,
    pub table: crate::table_relational::RelationalAdmissionEnvelope,
    pub graph: crate::graph_query_composition::GraphQueryResultFacade,
    pub lookup: crate::storage_index::LookupResultAdmission,
    pub require_table: bool,
    pub require_graph: bool,
    pub require_lookup: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CompositeReceiptEnvelope {
    pub header: ReceiptHeaderEnvelope,
    pub serialization: crate::serialization_codec::RoundTripEnvelope,
    pub validation_window: ValidationWindowEnvelope,
    pub evidence: ReceiptEvidenceEnvelope,
    pub all_artifacts_declared: bool,
    pub evidence_window_bounded: bool,
}

// r[depends composite-primitives.receipt-envelope]
pub open spec fn receipt_header_envelope_spec(e: ReceiptHeaderEnvelope) -> bool {
    crate::serialization_codec::schema_version_admitted_spec(e.schema)
        && e.section_count > 0
        && e.section_count <= e.max_sections
        && e.artifact_count <= e.max_artifacts
        && e.evidence_count <= e.max_evidence
}

// r[impl composite-primitives.receipt-envelope]
pub fn receipt_header_envelope_valid(e: ReceiptHeaderEnvelope) -> (result: bool)
    ensures result == receipt_header_envelope_spec(e)
{
    crate::serialization_codec::schema_version_admitted(e.schema)
        && e.section_count > 0
        && e.section_count <= e.max_sections
        && e.artifact_count <= e.max_artifacts
        && e.evidence_count <= e.max_evidence
}

// r[verify composite-primitives.receipt-envelope]
pub proof fn valid_receipt_header_has_supported_schema(e: ReceiptHeaderEnvelope)
    requires receipt_header_envelope_spec(e)
    ensures crate::serialization_codec::schema_version_admitted_spec(e.schema), e.section_count <= e.max_sections
{}

// r[depends composite-primitives.receipt-envelope]
pub open spec fn artifact_reference_envelope_spec(e: ArtifactReferenceEnvelope) -> bool {
    e.declared
        && e.artifact_index < e.artifact_count
        && crate::serialization_codec::byte_range_in_buffer_spec(e.bytes, e.buffer_len)
        && crate::storage_index::segment_map_spec(e.segment)
        && e.bytes.end <= e.segment.address_space_len
}

// r[impl composite-primitives.receipt-envelope]
pub fn artifact_reference_envelope_valid(e: ArtifactReferenceEnvelope) -> (result: bool)
    ensures result == artifact_reference_envelope_spec(e)
{
    e.declared
        && e.artifact_index < e.artifact_count
        && crate::serialization_codec::byte_range_in_buffer(e.bytes, e.buffer_len)
        && crate::storage_index::segment_map_valid(e.segment)
        && e.bytes.end <= e.segment.address_space_len
}

// r[verify composite-primitives.receipt-envelope]
pub proof fn valid_artifact_reference_is_declared_and_bounded(e: ArtifactReferenceEnvelope)
    requires artifact_reference_envelope_spec(e)
    ensures e.declared, e.artifact_index < e.artifact_count, e.bytes.end <= e.buffer_len
{}

// r[depends composite-primitives.receipt-envelope]
pub open spec fn validation_window_envelope_spec(e: ValidationWindowEnvelope) -> bool {
    e.start_tick <= e.result_tick
        && e.result_tick <= e.end_tick
        && e.end_tick - e.start_tick <= e.max_duration
        && crate::storage_index::cursor_bounds_spec(e.cursor)
}

// r[impl composite-primitives.receipt-envelope]
pub fn validation_window_envelope_valid(e: ValidationWindowEnvelope) -> (result: bool)
    ensures result == validation_window_envelope_spec(e)
{
    e.start_tick <= e.result_tick
        && e.result_tick <= e.end_tick
        && e.end_tick - e.start_tick <= e.max_duration
        && crate::storage_index::cursor_bounds_valid(e.cursor)
}

// r[verify composite-primitives.receipt-envelope]
pub proof fn valid_window_places_result_inside_bounds(e: ValidationWindowEnvelope)
    requires validation_window_envelope_spec(e)
    ensures e.start_tick <= e.result_tick, e.result_tick <= e.end_tick
{}

// r[depends composite-primitives.receipt-envelope]
pub open spec fn receipt_evidence_envelope_spec(e: ReceiptEvidenceEnvelope) -> bool {
    artifact_reference_envelope_spec(e.artifact)
        && (if e.require_table { crate::table_relational::relational_admission_envelope_spec(e.table) } else { true })
        && (if e.require_graph { crate::graph_query_composition::graph_query_result_facade_spec(e.graph) } else { true })
        && (if e.require_lookup { crate::storage_index::lookup_result_admission_spec(e.lookup) } else { true })
}

// r[impl composite-primitives.receipt-envelope]
pub fn receipt_evidence_envelope_valid(e: ReceiptEvidenceEnvelope) -> (result: bool)
    ensures result == receipt_evidence_envelope_spec(e)
{
    if !artifact_reference_envelope_valid(e.artifact) {
        return false;
    }
    if e.require_table && !crate::table_relational::relational_admission_envelope_valid(e.table) {
        return false;
    }
    if e.require_graph && !crate::graph_query_composition::graph_query_result_facade_valid(e.graph) {
        return false;
    }
    if e.require_lookup && !crate::storage_index::lookup_result_admission_valid(e.lookup) {
        return false;
    }
    true
}

// r[verify composite-primitives.receipt-envelope]
pub proof fn accepted_query_evidence_carries_source_witnesses(e: ReceiptEvidenceEnvelope)
    requires receipt_evidence_envelope_spec(e), e.require_graph, e.require_table
    ensures
        crate::graph_query_composition::graph_query_result_facade_spec(e.graph),
        crate::table_relational::relational_admission_envelope_spec(e.table),
        artifact_reference_envelope_spec(e.artifact)
{}

// r[depends composite-primitives.receipt-envelope]
pub open spec fn composite_receipt_envelope_spec(e: CompositeReceiptEnvelope) -> bool {
    receipt_header_envelope_spec(e.header)
        && crate::serialization_codec::bounded_round_trip_spec(e.serialization)
        && validation_window_envelope_spec(e.validation_window)
        && receipt_evidence_envelope_spec(e.evidence)
        && e.all_artifacts_declared
        && e.evidence_window_bounded
        && e.evidence.artifact.artifact_count == e.header.artifact_count
}

// r[impl composite-primitives.receipt-envelope]
pub fn composite_receipt_envelope_valid(e: CompositeReceiptEnvelope) -> (result: bool)
    ensures result == composite_receipt_envelope_spec(e)
{
    if !receipt_header_envelope_valid(e.header) {
        return false;
    }
    if !crate::serialization_codec::bounded_round_trip_admitted(e.serialization) {
        return false;
    }
    if !validation_window_envelope_valid(e.validation_window) {
        return false;
    }
    if !receipt_evidence_envelope_valid(e.evidence) {
        return false;
    }
    e.all_artifacts_declared && e.evidence_window_bounded && e.evidence.artifact.artifact_count == e.header.artifact_count
}

// r[verify composite-primitives.receipt-envelope]
pub proof fn accepted_receipt_has_declared_bounded_artifact(e: CompositeReceiptEnvelope)
    requires composite_receipt_envelope_spec(e)
    ensures
        e.all_artifacts_declared,
        e.evidence_window_bounded,
        artifact_reference_envelope_spec(e.evidence.artifact),
        e.evidence.artifact.artifact_count == e.header.artifact_count
{}

// r[verify composite-primitives.receipt-envelope]
pub proof fn dangling_artifact_reference_rejected(e: CompositeReceiptEnvelope)
    requires !artifact_reference_envelope_spec(e.evidence.artifact)
    ensures !composite_receipt_envelope_spec(e)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> crate::serialization_codec::SchemaVersionEnvelope {
        crate::serialization_codec::SchemaVersionEnvelope {
            schema_id: 7,
            expected_schema_id: 7,
            version: 1,
            min_supported_version: 1,
            max_supported_version: 2,
        }
    }

    fn artifact(declared: bool) -> ArtifactReferenceEnvelope {
        ArtifactReferenceEnvelope {
            artifact_index: 0,
            artifact_count: 1,
            bytes: crate::serialization_codec::ByteRange { start: 0, end: 8 },
            buffer_len: 8,
            segment: crate::storage_index::SegmentMap {
                segment_start: 0,
                segment_end: 8,
                address_space_len: 8,
            },
            declared,
        }
    }

    fn table() -> crate::table_relational::RelationalAdmissionEnvelope {
        crate::table_relational::RelationalAdmissionEnvelope {
            schema_ok: true,
            row_ok: true,
            key_ok: true,
            projection_ok: true,
            join_ok: true,
            grouping_ok: true,
        }
    }

    fn graph() -> crate::graph_query_composition::GraphQueryResultFacade {
        crate::graph_query_composition::GraphQueryResultFacade {
            model_ok: true,
            pattern_ok: true,
            binding_ok: true,
            path_ok: true,
            join_filter_ok: true,
            index_equiv_ok: true,
            serialized_envelope_ok: true,
        }
    }

    fn receipt(declared: bool) -> CompositeReceiptEnvelope {
        CompositeReceiptEnvelope {
            header: ReceiptHeaderEnvelope {
                schema: schema(),
                section_count: 3,
                max_sections: 4,
                artifact_count: 1,
                max_artifacts: 4,
                evidence_count: 1,
                max_evidence: 4,
            },
            serialization: crate::serialization_codec::RoundTripEnvelope {
                model_admitted: true,
                encoder_shape_admitted: true,
                decoder_shape_admitted: true,
                same_schema: true,
                same_version: true,
                output_len: 8,
                max_output_len: 16,
            },
            validation_window: ValidationWindowEnvelope {
                start_tick: 10,
                end_tick: 20,
                result_tick: 15,
                max_duration: 20,
                cursor: crate::storage_index::CursorBounds {
                    position: 0,
                    len: 1,
                    max_len: 4,
                },
            },
            evidence: ReceiptEvidenceEnvelope {
                artifact: artifact(declared),
                table: table(),
                graph: graph(),
                lookup: crate::storage_index::LookupResultAdmission {
                    found: true,
                    slot_index: 0,
                    slot_count: 1,
                },
                require_table: true,
                require_graph: true,
                require_lookup: true,
            },
            all_artifacts_declared: true,
            evidence_window_bounded: true,
        }
    }

    #[test]
    // r[verify composite-primitives.receipt-envelope]
    fn well_formed_receipt_is_admitted() {
        assert!(composite_receipt_envelope_valid(receipt(true)));
    }

    #[test]
    // r[verify composite-primitives.receipt-envelope]
    fn dangling_artifact_reference_is_rejected() {
        assert!(!composite_receipt_envelope_valid(receipt(false)));
    }

    #[test]
    // r[verify composite-primitives.receipt-envelope]
    fn out_of_window_result_is_rejected() {
        let mut r = receipt(true);
        r.validation_window.result_tick = 25;
        assert!(!composite_receipt_envelope_valid(r));
    }
}
