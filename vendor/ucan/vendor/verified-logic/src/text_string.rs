//! Exact finite text string primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FiniteByteString {
    pub len: usize,
    pub capacity: usize,
    pub max_capacity: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Utf8BoundaryRange {
    pub start: usize,
    pub end: usize,
    pub len: usize,
    pub start_boundary: bool,
    pub end_boundary: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TokenSpan {
    pub token_start: usize,
    pub token_end: usize,
    pub token_count: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PrefixSuffixPredicate {
    pub prefix_len: usize,
    pub suffix_len: usize,
    pub len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SubstringSpan {
    pub start: usize,
    pub end: usize,
    pub len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EditDistanceEnvelope {
    pub distance: usize,
    pub max_distance: usize,
    pub left_len: usize,
    pub right_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NormalizationEnvelope {
    pub input_len: usize,
    pub max_input_len: usize,
    pub output_len: usize,
    pub max_output_len: usize,
    pub form_id: u64,
    pub max_form_id: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DelimiterSplitJoinShape {
    pub part_count: usize,
    pub max_parts: usize,
    pub delimiter_len: usize,
    pub joined_len: usize,
    pub part_bytes: usize,
}

// r[depends text.finite-byte-string]
pub open spec fn finite_byte_string_spec(e: FiniteByteString) -> bool {
    e.len <= e.capacity && e.capacity <= e.max_capacity
}

// r[impl text.finite-byte-string]
pub fn finite_byte_string_valid(e: FiniteByteString) -> (result: bool)
    ensures result == finite_byte_string_spec(e)
{
    e.len <= e.capacity && e.capacity <= e.max_capacity
}

// r[verify text.finite-byte-string]
pub proof fn valid_finite_byte_string_is_admitted(e: FiniteByteString)
    requires finite_byte_string_spec(e)
    ensures finite_byte_string_spec(e)
{}

// r[depends text.utf8-boundary-range]
pub open spec fn utf8_boundary_range_spec(e: Utf8BoundaryRange) -> bool {
    e.start <= e.end && e.end <= e.len && e.start_boundary && e.end_boundary
}

// r[impl text.utf8-boundary-range]
pub fn utf8_boundary_range_valid(e: Utf8BoundaryRange) -> (result: bool)
    ensures result == utf8_boundary_range_spec(e)
{
    e.start <= e.end && e.end <= e.len && e.start_boundary && e.end_boundary
}

// r[verify text.utf8-boundary-range]
pub proof fn valid_utf8_boundary_range_is_admitted(e: Utf8BoundaryRange)
    requires utf8_boundary_range_spec(e)
    ensures utf8_boundary_range_spec(e)
{}

// r[depends text.token-span]
pub open spec fn token_span_spec(e: TokenSpan) -> bool {
    e.token_start <= e.token_end && e.token_end <= e.token_count && e.byte_start <= e.byte_end
}

// r[impl text.token-span]
pub fn token_span_valid(e: TokenSpan) -> (result: bool)
    ensures result == token_span_spec(e)
{
    e.token_start <= e.token_end && e.token_end <= e.token_count && e.byte_start <= e.byte_end
}

// r[verify text.token-span]
pub proof fn valid_token_span_is_admitted(e: TokenSpan)
    requires token_span_spec(e)
    ensures token_span_spec(e)
{}

// r[depends text.prefix-suffix]
pub open spec fn prefix_suffix_predicate_spec(e: PrefixSuffixPredicate) -> bool {
    e.prefix_len <= e.len && e.suffix_len <= e.len - e.prefix_len
}

// r[impl text.prefix-suffix]
pub fn prefix_suffix_predicate_valid(e: PrefixSuffixPredicate) -> (result: bool)
    ensures result == prefix_suffix_predicate_spec(e)
{
    e.prefix_len <= e.len && e.suffix_len <= e.len - e.prefix_len
}

// r[verify text.prefix-suffix]
pub proof fn valid_prefix_suffix_predicate_is_admitted(e: PrefixSuffixPredicate)
    requires prefix_suffix_predicate_spec(e)
    ensures prefix_suffix_predicate_spec(e)
{}

// r[depends text.substring-span]
pub open spec fn substring_span_spec(e: SubstringSpan) -> bool {
    e.start <= e.end && e.end <= e.len
}

// r[impl text.substring-span]
pub fn substring_span_valid(e: SubstringSpan) -> (result: bool)
    ensures result == substring_span_spec(e)
{
    e.start <= e.end && e.end <= e.len
}

// r[verify text.substring-span]
pub proof fn valid_substring_span_is_admitted(e: SubstringSpan)
    requires substring_span_spec(e)
    ensures substring_span_spec(e)
{}

// r[depends text.edit-distance-envelope]
pub open spec fn edit_distance_envelope_spec(e: EditDistanceEnvelope) -> bool {
    e.distance <= e.max_distance && e.distance <= e.left_len && e.distance <= e.right_len
}

// r[impl text.edit-distance-envelope]
pub fn edit_distance_envelope_valid(e: EditDistanceEnvelope) -> (result: bool)
    ensures result == edit_distance_envelope_spec(e)
{
    e.distance <= e.max_distance && e.distance <= e.left_len && e.distance <= e.right_len
}

// r[verify text.edit-distance-envelope]
pub proof fn valid_edit_distance_envelope_is_admitted(e: EditDistanceEnvelope)
    requires edit_distance_envelope_spec(e)
    ensures edit_distance_envelope_spec(e)
{}

// r[depends text.normalization-envelope]
pub open spec fn normalization_envelope_spec(e: NormalizationEnvelope) -> bool {
    e.input_len <= e.max_input_len && e.output_len <= e.max_output_len && e.form_id <= e.max_form_id
}

// r[impl text.normalization-envelope]
pub fn normalization_envelope_valid(e: NormalizationEnvelope) -> (result: bool)
    ensures result == normalization_envelope_spec(e)
{
    e.input_len <= e.max_input_len && e.output_len <= e.max_output_len && e.form_id <= e.max_form_id
}

// r[verify text.normalization-envelope]
pub proof fn valid_normalization_envelope_is_admitted(e: NormalizationEnvelope)
    requires normalization_envelope_spec(e)
    ensures normalization_envelope_spec(e)
{}

// r[depends text.delimiter-split-join]
pub open spec fn delimiter_split_join_shape_spec(e: DelimiterSplitJoinShape) -> bool {
    e.part_count <= e.max_parts && e.delimiter_len > 0 && e.joined_len >= e.part_bytes
}

// r[impl text.delimiter-split-join]
pub fn delimiter_split_join_shape_valid(e: DelimiterSplitJoinShape) -> (result: bool)
    ensures result == delimiter_split_join_shape_spec(e)
{
    e.part_count <= e.max_parts && e.delimiter_len > 0 && e.joined_len >= e.part_bytes
}

// r[verify text.delimiter-split-join]
pub proof fn valid_delimiter_split_join_shape_is_admitted(e: DelimiterSplitJoinShape)
    requires delimiter_split_join_shape_spec(e)
    ensures delimiter_split_join_shape_spec(e)
{}

} // verus!
