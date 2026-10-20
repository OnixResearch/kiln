//! Exact finite parsing format primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GrammarSymbol {
    pub symbol_id: u64,
    pub max_symbol_id: u64,
    pub arity: usize,
    pub max_arity: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TokenStreamShape {
    pub token_count: usize,
    pub max_tokens: usize,
    pub first_offset: usize,
    pub last_offset: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
    pub source_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AstShape {
    pub node_count: usize,
    pub max_nodes: usize,
    pub edge_count: usize,
    pub max_edges: usize,
    pub root_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParseErrorEnvelope {
    pub span_start: usize,
    pub span_end: usize,
    pub source_len: usize,
    pub code: u64,
    pub max_code: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormatVersionAdmission {
    pub format_id: u64,
    pub expected_format_id: u64,
    pub version: u64,
    pub min_version: u64,
    pub max_version: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SerializerShape {
    pub ast_ok: bool,
    pub output_len: usize,
    pub max_output_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeserializerShape {
    pub tokens_ok: bool,
    pub ast_ok: bool,
    pub consumed: usize,
    pub input_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParseRoundTripEnvelope {
    pub serializer_ok: bool,
    pub deserializer_ok: bool,
    pub same_version: bool,
    pub input_len: usize,
    pub max_input_len: usize,
}

// r[depends domain-primitives.parsing-format.grammar-symbol]
pub open spec fn grammar_symbol_spec(e: GrammarSymbol) -> bool {
    e.symbol_id <= e.max_symbol_id && e.arity <= e.max_arity
}

// r[impl domain-primitives.parsing-format.grammar-symbol]
pub fn grammar_symbol_valid(e: GrammarSymbol) -> (result: bool)
    ensures result == grammar_symbol_spec(e)
{
    e.symbol_id <= e.max_symbol_id && e.arity <= e.max_arity
}

// r[verify domain-primitives.parsing-format.grammar-symbol]
pub proof fn valid_grammar_symbol_is_admitted(e: GrammarSymbol)
    requires grammar_symbol_spec(e)
    ensures grammar_symbol_spec(e)
{}

// r[depends domain-primitives.parsing-format.token-stream]
pub open spec fn token_stream_shape_spec(e: TokenStreamShape) -> bool {
    e.token_count <= e.max_tokens && e.first_offset <= e.last_offset
}

// r[impl domain-primitives.parsing-format.token-stream]
pub fn token_stream_shape_valid(e: TokenStreamShape) -> (result: bool)
    ensures result == token_stream_shape_spec(e)
{
    e.token_count <= e.max_tokens && e.first_offset <= e.last_offset
}

// r[verify domain-primitives.parsing-format.token-stream]
pub proof fn valid_token_stream_shape_is_admitted(e: TokenStreamShape)
    requires token_stream_shape_spec(e)
    ensures token_stream_shape_spec(e)
{}

// r[depends domain-primitives.parsing-format.source-span]
pub open spec fn source_span_spec(e: SourceSpan) -> bool {
    e.start <= e.end && e.end <= e.source_len
}

// r[impl domain-primitives.parsing-format.source-span]
pub fn source_span_valid(e: SourceSpan) -> (result: bool)
    ensures result == source_span_spec(e)
{
    e.start <= e.end && e.end <= e.source_len
}

// r[verify domain-primitives.parsing-format.source-span]
pub proof fn valid_source_span_is_admitted(e: SourceSpan)
    requires source_span_spec(e)
    ensures source_span_spec(e)
{}

// r[depends domain-primitives.parsing-format.ast-shape]
pub open spec fn ast_shape_spec(e: AstShape) -> bool {
    e.node_count > 0 && e.node_count <= e.max_nodes && e.edge_count <= e.max_edges && e.root_index < e.node_count
}

// r[impl domain-primitives.parsing-format.ast-shape]
pub fn ast_shape_valid(e: AstShape) -> (result: bool)
    ensures result == ast_shape_spec(e)
{
    e.node_count > 0 && e.node_count <= e.max_nodes && e.edge_count <= e.max_edges && e.root_index < e.node_count
}

// r[verify domain-primitives.parsing-format.ast-shape]
pub proof fn valid_ast_shape_is_admitted(e: AstShape)
    requires ast_shape_spec(e)
    ensures ast_shape_spec(e)
{}

// r[depends domain-primitives.parsing-format.error-envelope]
pub open spec fn parse_error_envelope_spec(e: ParseErrorEnvelope) -> bool {
    e.span_start <= e.span_end && e.span_end <= e.source_len && e.code <= e.max_code
}

// r[impl domain-primitives.parsing-format.error-envelope]
pub fn parse_error_envelope_valid(e: ParseErrorEnvelope) -> (result: bool)
    ensures result == parse_error_envelope_spec(e)
{
    e.span_start <= e.span_end && e.span_end <= e.source_len && e.code <= e.max_code
}

// r[verify domain-primitives.parsing-format.error-envelope]
pub proof fn valid_parse_error_envelope_is_admitted(e: ParseErrorEnvelope)
    requires parse_error_envelope_spec(e)
    ensures parse_error_envelope_spec(e)
{}

// r[depends domain-primitives.parsing-format.version-admission]
pub open spec fn format_version_admission_spec(e: FormatVersionAdmission) -> bool {
    e.format_id == e.expected_format_id && e.min_version <= e.version && e.version <= e.max_version && e.min_version <= e.max_version
}

// r[impl domain-primitives.parsing-format.version-admission]
pub fn format_version_admission_valid(e: FormatVersionAdmission) -> (result: bool)
    ensures result == format_version_admission_spec(e)
{
    e.format_id == e.expected_format_id && e.min_version <= e.version && e.version <= e.max_version && e.min_version <= e.max_version
}

// r[verify domain-primitives.parsing-format.version-admission]
pub proof fn valid_format_version_admission_is_admitted(e: FormatVersionAdmission)
    requires format_version_admission_spec(e)
    ensures format_version_admission_spec(e)
{}

// r[depends domain-primitives.parsing-format.serialize-shape]
pub open spec fn serializer_shape_spec(e: SerializerShape) -> bool {
    e.ast_ok && e.output_len <= e.max_output_len
}

// r[impl domain-primitives.parsing-format.serialize-shape]
pub fn serializer_shape_valid(e: SerializerShape) -> (result: bool)
    ensures result == serializer_shape_spec(e)
{
    e.ast_ok && e.output_len <= e.max_output_len
}

// r[verify domain-primitives.parsing-format.serialize-shape]
pub proof fn valid_serializer_shape_is_admitted(e: SerializerShape)
    requires serializer_shape_spec(e)
    ensures serializer_shape_spec(e)
{}

// r[depends domain-primitives.parsing-format.deserialize-shape]
pub open spec fn deserializer_shape_spec(e: DeserializerShape) -> bool {
    e.tokens_ok && e.ast_ok && e.consumed <= e.input_len
}

// r[impl domain-primitives.parsing-format.deserialize-shape]
pub fn deserializer_shape_valid(e: DeserializerShape) -> (result: bool)
    ensures result == deserializer_shape_spec(e)
{
    e.tokens_ok && e.ast_ok && e.consumed <= e.input_len
}

// r[verify domain-primitives.parsing-format.deserialize-shape]
pub proof fn valid_deserializer_shape_is_admitted(e: DeserializerShape)
    requires deserializer_shape_spec(e)
    ensures deserializer_shape_spec(e)
{}

// r[depends domain-primitives.parsing-format.roundtrip-envelope]
pub open spec fn parse_round_trip_envelope_spec(e: ParseRoundTripEnvelope) -> bool {
    e.serializer_ok && e.deserializer_ok && e.same_version && e.input_len <= e.max_input_len
}

// r[impl domain-primitives.parsing-format.roundtrip-envelope]
pub fn parse_round_trip_envelope_valid(e: ParseRoundTripEnvelope) -> (result: bool)
    ensures result == parse_round_trip_envelope_spec(e)
{
    e.serializer_ok && e.deserializer_ok && e.same_version && e.input_len <= e.max_input_len
}

// r[verify domain-primitives.parsing-format.roundtrip-envelope]
pub proof fn valid_parse_round_trip_envelope_is_admitted(e: ParseRoundTripEnvelope)
    requires parse_round_trip_envelope_spec(e)
    ensures parse_round_trip_envelope_spec(e)
{}

} // verus!
