//! Exact finite table relational primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TableSchemaMetadata {
    pub column_count: usize,
    pub max_columns: usize,
    pub row_count: usize,
    pub max_rows: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColumnMetadataEnvelope {
    pub column_index: usize,
    pub column_count: usize,
    pub type_id: u64,
    pub max_type_id: u64,
    pub nullable: bool,
    pub default_present: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RowShapeEnvelope {
    pub cell_count: usize,
    pub column_count: usize,
    pub null_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyMetadataEnvelope {
    pub key_column_count: usize,
    pub column_count: usize,
    pub unique_count: usize,
    pub row_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProjectionFilterShape {
    pub projected_columns: usize,
    pub input_columns: usize,
    pub predicate_count: usize,
    pub max_predicates: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct JoinShapeMetadata {
    pub left_keys: usize,
    pub right_keys: usize,
    pub left_columns: usize,
    pub right_columns: usize,
    pub output_columns: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GroupingMetadataEnvelope {
    pub group_key_count: usize,
    pub input_columns: usize,
    pub aggregate_count: usize,
    pub max_aggregates: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RelationalAdmissionEnvelope {
    pub schema_ok: bool,
    pub row_ok: bool,
    pub key_ok: bool,
    pub projection_ok: bool,
    pub join_ok: bool,
    pub grouping_ok: bool,
}

// r[depends domain-primitives.table-relational.table-schema]
pub open spec fn table_schema_metadata_spec(e: TableSchemaMetadata) -> bool {
    e.column_count > 0 && e.column_count <= e.max_columns && e.row_count <= e.max_rows
}

// r[impl domain-primitives.table-relational.table-schema]
pub fn table_schema_metadata_valid(e: TableSchemaMetadata) -> (result: bool)
    ensures result == table_schema_metadata_spec(e)
{
    e.column_count > 0 && e.column_count <= e.max_columns && e.row_count <= e.max_rows
}

// r[verify domain-primitives.table-relational.table-schema]
pub proof fn valid_table_schema_metadata_is_admitted(e: TableSchemaMetadata)
    requires table_schema_metadata_spec(e)
    ensures table_schema_metadata_spec(e)
{}

// r[depends domain-primitives.table-relational.column]
pub open spec fn column_metadata_envelope_spec(e: ColumnMetadataEnvelope) -> bool {
    e.column_index < e.column_count && e.type_id <= e.max_type_id && (!e.nullable || e.default_present)
}

// r[impl domain-primitives.table-relational.column]
pub fn column_metadata_envelope_valid(e: ColumnMetadataEnvelope) -> (result: bool)
    ensures result == column_metadata_envelope_spec(e)
{
    e.column_index < e.column_count && e.type_id <= e.max_type_id && (!e.nullable || e.default_present)
}

// r[verify domain-primitives.table-relational.column]
pub proof fn valid_column_metadata_envelope_is_admitted(e: ColumnMetadataEnvelope)
    requires column_metadata_envelope_spec(e)
    ensures column_metadata_envelope_spec(e)
{}

// r[depends domain-primitives.table-relational.row]
pub open spec fn row_shape_envelope_spec(e: RowShapeEnvelope) -> bool {
    e.cell_count == e.column_count && e.null_count <= e.cell_count
}

// r[impl domain-primitives.table-relational.row]
pub fn row_shape_envelope_valid(e: RowShapeEnvelope) -> (result: bool)
    ensures result == row_shape_envelope_spec(e)
{
    e.cell_count == e.column_count && e.null_count <= e.cell_count
}

// r[verify domain-primitives.table-relational.row]
pub proof fn valid_row_shape_envelope_is_admitted(e: RowShapeEnvelope)
    requires row_shape_envelope_spec(e)
    ensures row_shape_envelope_spec(e)
{}

// r[depends domain-primitives.table-relational.key]
pub open spec fn key_metadata_envelope_spec(e: KeyMetadataEnvelope) -> bool {
    e.key_column_count > 0 && e.key_column_count <= e.column_count && e.unique_count <= e.row_count
}

// r[impl domain-primitives.table-relational.key]
pub fn key_metadata_envelope_valid(e: KeyMetadataEnvelope) -> (result: bool)
    ensures result == key_metadata_envelope_spec(e)
{
    e.key_column_count > 0 && e.key_column_count <= e.column_count && e.unique_count <= e.row_count
}

// r[verify domain-primitives.table-relational.key]
pub proof fn valid_key_metadata_envelope_is_admitted(e: KeyMetadataEnvelope)
    requires key_metadata_envelope_spec(e)
    ensures key_metadata_envelope_spec(e)
{}

// r[depends domain-primitives.table-relational.projection-filter]
pub open spec fn projection_filter_shape_spec(e: ProjectionFilterShape) -> bool {
    e.projected_columns <= e.input_columns && e.predicate_count <= e.max_predicates
}

// r[impl domain-primitives.table-relational.projection-filter]
pub fn projection_filter_shape_valid(e: ProjectionFilterShape) -> (result: bool)
    ensures result == projection_filter_shape_spec(e)
{
    e.projected_columns <= e.input_columns && e.predicate_count <= e.max_predicates
}

// r[verify domain-primitives.table-relational.projection-filter]
pub proof fn valid_projection_filter_shape_is_admitted(e: ProjectionFilterShape)
    requires projection_filter_shape_spec(e)
    ensures projection_filter_shape_spec(e)
{}

// r[depends domain-primitives.table-relational.join]
pub open spec fn join_shape_metadata_spec(e: JoinShapeMetadata) -> bool {
    e.left_keys == e.right_keys && e.left_keys > 0 && e.output_columns >= e.left_columns && e.output_columns >= e.right_columns
}

// r[impl domain-primitives.table-relational.join]
pub fn join_shape_metadata_valid(e: JoinShapeMetadata) -> (result: bool)
    ensures result == join_shape_metadata_spec(e)
{
    e.left_keys == e.right_keys && e.left_keys > 0 && e.output_columns >= e.left_columns && e.output_columns >= e.right_columns
}

// r[verify domain-primitives.table-relational.join]
pub proof fn valid_join_shape_metadata_is_admitted(e: JoinShapeMetadata)
    requires join_shape_metadata_spec(e)
    ensures join_shape_metadata_spec(e)
{}

// r[depends domain-primitives.table-relational.grouping]
pub open spec fn grouping_metadata_envelope_spec(e: GroupingMetadataEnvelope) -> bool {
    e.group_key_count <= e.input_columns && e.aggregate_count <= e.max_aggregates
}

// r[impl domain-primitives.table-relational.grouping]
pub fn grouping_metadata_envelope_valid(e: GroupingMetadataEnvelope) -> (result: bool)
    ensures result == grouping_metadata_envelope_spec(e)
{
    e.group_key_count <= e.input_columns && e.aggregate_count <= e.max_aggregates
}

// r[verify domain-primitives.table-relational.grouping]
pub proof fn valid_grouping_metadata_envelope_is_admitted(e: GroupingMetadataEnvelope)
    requires grouping_metadata_envelope_spec(e)
    ensures grouping_metadata_envelope_spec(e)
{}

// r[depends domain-primitives.table-relational.admission]
pub open spec fn relational_admission_envelope_spec(e: RelationalAdmissionEnvelope) -> bool {
    e.schema_ok && e.row_ok && e.key_ok && e.projection_ok && e.join_ok && e.grouping_ok
}

// r[impl domain-primitives.table-relational.admission]
pub fn relational_admission_envelope_valid(e: RelationalAdmissionEnvelope) -> (result: bool)
    ensures result == relational_admission_envelope_spec(e)
{
    e.schema_ok && e.row_ok && e.key_ok && e.projection_ok && e.join_ok && e.grouping_ok
}

// r[verify domain-primitives.table-relational.admission]
pub proof fn valid_relational_admission_envelope_is_admitted(e: RelationalAdmissionEnvelope)
    requires relational_admission_envelope_spec(e)
    ensures relational_admission_envelope_spec(e)
{}

} // verus!
