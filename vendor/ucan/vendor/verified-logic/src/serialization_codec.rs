//! Exact finite serialization/codec primitive checks.
#![allow(clippy::struct_excessive_bools)]
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ByteBufferEnvelope {
    pub len: usize,
    pub capacity: usize,
    pub min_byte: u8,
    pub max_byte: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameEnvelope {
    pub buffer_len: usize,
    pub header: ByteRange,
    pub payload: ByteRange,
    pub trailer: ByteRange,
    pub has_trailer: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LengthPrefixEnvelope {
    pub prefix: ByteRange,
    pub remaining: ByteRange,
    pub declared_len: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaVersionEnvelope {
    pub schema_id: u64,
    pub expected_schema_id: u64,
    pub version: u64,
    pub min_supported_version: u64,
    pub max_supported_version: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChecksumShapeEnvelope {
    pub buffer_len: usize,
    pub algorithm_id: u64,
    pub min_algorithm_id: u64,
    pub max_algorithm_id: u64,
    pub checksum: ByteRange,
    pub covered: ByteRange,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EncoderShapeEnvelope {
    pub model_len: usize,
    pub max_model_len: usize,
    pub output_len: usize,
    pub max_output_len: usize,
    pub frame: FrameEnvelope,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DecoderShapeEnvelope {
    pub buffer: ByteBufferEnvelope,
    pub frame: FrameEnvelope,
    pub length_prefix: LengthPrefixEnvelope,
    pub schema: SchemaVersionEnvelope,
    pub checksum: ChecksumShapeEnvelope,
    pub require_checksum: bool,
    pub checksum_present: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RoundTripEnvelope {
    pub model_admitted: bool,
    pub encoder_shape_admitted: bool,
    pub decoder_shape_admitted: bool,
    pub same_schema: bool,
    pub same_version: bool,
    pub output_len: usize,
    pub max_output_len: usize,
}

// r[depends domain-primitives.serialization-codec.byte-buffer]
pub open spec fn byte_range_in_buffer_spec(range: ByteRange, len: usize) -> bool {
    range.start <= range.end && range.end <= len
}

// r[depends domain-primitives.serialization-codec.byte-buffer]
pub open spec fn byte_buffer_envelope_spec(buffer: ByteBufferEnvelope) -> bool {
    buffer.len <= buffer.capacity && buffer.min_byte <= buffer.max_byte
}

// r[impl domain-primitives.serialization-codec.byte-buffer]
pub fn byte_range_in_buffer(range: ByteRange, len: usize) -> (result: bool)
    ensures result == byte_range_in_buffer_spec(range, len)
{
    range.start <= range.end && range.end <= len
}

// r[impl domain-primitives.serialization-codec.byte-buffer]
pub fn byte_buffer_envelope_valid(buffer: ByteBufferEnvelope) -> (result: bool)
    ensures result == byte_buffer_envelope_spec(buffer)
{
    buffer.len <= buffer.capacity && buffer.min_byte <= buffer.max_byte
}

// r[verify domain-primitives.serialization-codec.byte-buffer]
pub proof fn admitted_range_end_fits_buffer(range: ByteRange, len: usize)
    requires byte_range_in_buffer_spec(range, len)
    ensures range.end <= len
{}

// r[depends domain-primitives.serialization-codec.frame]
pub open spec fn frame_envelope_spec(frame: FrameEnvelope) -> bool {
    byte_range_in_buffer_spec(frame.header, frame.buffer_len)
        && byte_range_in_buffer_spec(frame.payload, frame.buffer_len)
        && byte_range_in_buffer_spec(frame.trailer, frame.buffer_len)
        && frame.header.end <= frame.payload.start
        && (if frame.has_trailer { frame.payload.end <= frame.trailer.start } else { frame.trailer.start == frame.trailer.end })
}

// r[impl domain-primitives.serialization-codec.frame]
pub fn frame_envelope_valid(frame: FrameEnvelope) -> (result: bool)
    ensures result == frame_envelope_spec(frame)
{
    if !byte_range_in_buffer(frame.header, frame.buffer_len) {
        return false;
    }
    if !byte_range_in_buffer(frame.payload, frame.buffer_len) {
        return false;
    }
    if !byte_range_in_buffer(frame.trailer, frame.buffer_len) {
        return false;
    }
    if frame.header.end > frame.payload.start {
        return false;
    }
    if frame.has_trailer {
        frame.payload.end <= frame.trailer.start
    } else {
        frame.trailer.start == frame.trailer.end
    }
}

// r[verify domain-primitives.serialization-codec.frame]
pub proof fn valid_frame_payload_fits(frame: FrameEnvelope)
    requires frame_envelope_spec(frame)
    ensures frame.payload.end <= frame.buffer_len
{}

// r[depends domain-primitives.serialization-codec.length-prefix]
pub open spec fn length_prefix_envelope_spec(prefix: LengthPrefixEnvelope) -> bool {
    byte_range_in_buffer_spec(prefix.prefix, prefix.remaining.end)
        && byte_range_in_buffer_spec(prefix.remaining, prefix.remaining.end)
        && prefix.prefix.end <= prefix.remaining.start
        && prefix.declared_len <= prefix.remaining.end - prefix.remaining.start
}

// r[impl domain-primitives.serialization-codec.length-prefix]
pub fn length_prefix_envelope_valid(prefix: LengthPrefixEnvelope) -> (result: bool)
    ensures result == length_prefix_envelope_spec(prefix)
{
    if !byte_range_in_buffer(prefix.prefix, prefix.remaining.end) {
        return false;
    }
    if !byte_range_in_buffer(prefix.remaining, prefix.remaining.end) {
        return false;
    }
    if prefix.prefix.end > prefix.remaining.start {
        return false;
    }
    prefix.declared_len <= prefix.remaining.end - prefix.remaining.start
}

// r[verify domain-primitives.serialization-codec.length-prefix]
pub proof fn admitted_declared_length_fits(prefix: LengthPrefixEnvelope)
    requires length_prefix_envelope_spec(prefix)
    ensures prefix.declared_len <= prefix.remaining.end - prefix.remaining.start
{}

// r[depends domain-primitives.serialization-codec.schema-version]
pub open spec fn schema_version_admitted_spec(schema: SchemaVersionEnvelope) -> bool {
    schema.schema_id == schema.expected_schema_id
        && schema.min_supported_version <= schema.max_supported_version
        && schema.min_supported_version <= schema.version <= schema.max_supported_version
}

// r[impl domain-primitives.serialization-codec.schema-version]
pub fn schema_version_admitted(schema: SchemaVersionEnvelope) -> (result: bool)
    ensures result == schema_version_admitted_spec(schema)
{
    schema.schema_id == schema.expected_schema_id
        && schema.min_supported_version <= schema.max_supported_version
        && schema.min_supported_version <= schema.version
        && schema.version <= schema.max_supported_version
}

// r[verify domain-primitives.serialization-codec.schema-version]
pub proof fn unsupported_schema_rejected(schema: SchemaVersionEnvelope)
    requires schema.schema_id != schema.expected_schema_id
    ensures !schema_version_admitted_spec(schema)
{}

// r[depends domain-primitives.serialization-codec.checksum-shape]
pub open spec fn checksum_shape_spec(shape: ChecksumShapeEnvelope) -> bool {
    shape.min_algorithm_id <= shape.max_algorithm_id
        && shape.min_algorithm_id <= shape.algorithm_id <= shape.max_algorithm_id
        && byte_range_in_buffer_spec(shape.checksum, shape.buffer_len)
        && byte_range_in_buffer_spec(shape.covered, shape.buffer_len)
}

// r[impl domain-primitives.serialization-codec.checksum-shape]
pub fn checksum_shape_valid(shape: ChecksumShapeEnvelope) -> (result: bool)
    ensures result == checksum_shape_spec(shape)
{
    shape.min_algorithm_id <= shape.max_algorithm_id
        && shape.min_algorithm_id <= shape.algorithm_id
        && shape.algorithm_id <= shape.max_algorithm_id
        && byte_range_in_buffer(shape.checksum, shape.buffer_len)
        && byte_range_in_buffer(shape.covered, shape.buffer_len)
}

// r[verify domain-primitives.serialization-codec.checksum-shape]
pub proof fn checksum_shape_is_structural_only(shape: ChecksumShapeEnvelope)
    requires checksum_shape_spec(shape)
    ensures byte_range_in_buffer_spec(shape.checksum, shape.buffer_len), byte_range_in_buffer_spec(shape.covered, shape.buffer_len)
{}

// r[depends domain-primitives.serialization-codec.encoder-shape]
pub open spec fn encoder_shape_spec(shape: EncoderShapeEnvelope) -> bool {
    shape.model_len <= shape.max_model_len
        && shape.output_len <= shape.max_output_len
        && shape.frame.buffer_len == shape.output_len
        && frame_envelope_spec(shape.frame)
}

// r[impl domain-primitives.serialization-codec.encoder-shape]
pub fn encoder_shape_valid(shape: EncoderShapeEnvelope) -> (result: bool)
    ensures result == encoder_shape_spec(shape)
{
    shape.model_len <= shape.max_model_len
        && shape.output_len <= shape.max_output_len
        && shape.frame.buffer_len == shape.output_len
        && frame_envelope_valid(shape.frame)
}

// r[verify domain-primitives.serialization-codec.encoder-shape]
pub proof fn encoder_output_is_bounded(shape: EncoderShapeEnvelope)
    requires encoder_shape_spec(shape)
    ensures shape.output_len <= shape.max_output_len
{}

// r[depends domain-primitives.serialization-codec.decoder-shape]
pub open spec fn decoder_shape_spec(shape: DecoderShapeEnvelope) -> bool {
    byte_buffer_envelope_spec(shape.buffer)
        && shape.frame.buffer_len == shape.buffer.len
        && shape.length_prefix.remaining.end <= shape.buffer.len
        && shape.checksum.buffer_len == shape.buffer.len
        && frame_envelope_spec(shape.frame)
        && length_prefix_envelope_spec(shape.length_prefix)
        && schema_version_admitted_spec(shape.schema)
        && (if shape.require_checksum { shape.checksum_present && checksum_shape_spec(shape.checksum) } else { !shape.checksum_present || checksum_shape_spec(shape.checksum) })
}

// r[impl domain-primitives.serialization-codec.decoder-shape]
pub fn decoder_shape_valid(shape: DecoderShapeEnvelope) -> (result: bool)
    ensures result == decoder_shape_spec(shape)
{
    if !byte_buffer_envelope_valid(shape.buffer) {
        return false;
    }
    if shape.frame.buffer_len != shape.buffer.len || shape.length_prefix.remaining.end > shape.buffer.len || shape.checksum.buffer_len != shape.buffer.len {
        return false;
    }
    if !frame_envelope_valid(shape.frame) || !length_prefix_envelope_valid(shape.length_prefix) || !schema_version_admitted(shape.schema) {
        return false;
    }
    if shape.require_checksum {
        shape.checksum_present && checksum_shape_valid(shape.checksum)
    } else {
        !shape.checksum_present || checksum_shape_valid(shape.checksum)
    }
}

// r[verify domain-primitives.serialization-codec.decoder-shape]
pub proof fn decoder_shape_rejects_malformed_frame(shape: DecoderShapeEnvelope)
    requires !frame_envelope_spec(shape.frame)
    ensures !decoder_shape_spec(shape)
{}

// r[depends domain-primitives.serialization-codec.round-trip]
pub open spec fn bounded_round_trip_spec(rt: RoundTripEnvelope) -> bool {
    rt.model_admitted
        && rt.encoder_shape_admitted
        && rt.decoder_shape_admitted
        && rt.same_schema
        && rt.same_version
        && rt.output_len <= rt.max_output_len
}

// r[impl domain-primitives.serialization-codec.round-trip]
pub fn bounded_round_trip_admitted(rt: RoundTripEnvelope) -> (result: bool)
    ensures result == bounded_round_trip_spec(rt)
{
    rt.model_admitted
        && rt.encoder_shape_admitted
        && rt.decoder_shape_admitted
        && rt.same_schema
        && rt.same_version
        && rt.output_len <= rt.max_output_len
}

// r[verify domain-primitives.serialization-codec.round-trip]
pub proof fn round_trip_requires_admitted_model(rt: RoundTripEnvelope)
    requires bounded_round_trip_spec(rt)
    ensures rt.model_admitted
{}

// r[impl domain-primitives.serialization-codec.byte-buffer]
// r[impl domain-primitives.serialization-codec.frame]
// r[impl domain-primitives.serialization-codec.length-prefix]
// r[impl domain-primitives.serialization-codec.schema-version]
// r[impl domain-primitives.serialization-codec.checksum-shape]
// r[impl domain-primitives.serialization-codec.encoder-shape]
// r[impl domain-primitives.serialization-codec.decoder-shape]
// r[impl domain-primitives.serialization-codec.round-trip]
pub fn serialization_codec_non_claims() -> (result: bool)
    ensures result
{
    true
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: usize, end: usize) -> ByteRange {
        ByteRange { start, end }
    }

    #[test]
    fn byte_buffer_and_ranges_are_finite() {
        assert!(byte_buffer_envelope_valid(ByteBufferEnvelope {
            len: 8,
            capacity: 16,
            min_byte: 0,
            max_byte: 255
        }));
        assert!(!byte_buffer_envelope_valid(ByteBufferEnvelope {
            len: 17,
            capacity: 16,
            min_byte: 0,
            max_byte: 255
        }));
        assert!(byte_range_in_buffer(range(2, 4), 8));
        assert!(!byte_range_in_buffer(range(4, 9), 8));
    }

    #[test]
    fn frame_sections_stay_inside_buffer() {
        let frame = FrameEnvelope {
            buffer_len: 12,
            header: range(0, 2),
            payload: range(2, 10),
            trailer: range(10, 12),
            has_trailer: true,
        };
        assert!(frame_envelope_valid(frame));
        assert!(!frame_envelope_valid(FrameEnvelope {
            payload: range(1, 10),
            ..frame
        }));
        assert!(!frame_envelope_valid(FrameEnvelope {
            trailer: range(12, 13),
            ..frame
        }));
    }

    #[test]
    fn length_prefix_declared_length_fits_remaining_window() {
        assert!(length_prefix_envelope_valid(LengthPrefixEnvelope {
            prefix: range(0, 2),
            remaining: range(2, 8),
            declared_len: 6
        }));
        assert!(!length_prefix_envelope_valid(LengthPrefixEnvelope {
            prefix: range(0, 2),
            remaining: range(2, 8),
            declared_len: 7
        }));
    }

    #[test]
    fn schema_and_checksum_shape_are_deterministic() {
        let schema = SchemaVersionEnvelope {
            schema_id: 7,
            expected_schema_id: 7,
            version: 2,
            min_supported_version: 1,
            max_supported_version: 3,
        };
        assert!(schema_version_admitted(schema));
        assert!(!schema_version_admitted(SchemaVersionEnvelope { schema_id: 8, ..schema }));
        let checksum = ChecksumShapeEnvelope {
            buffer_len: 16,
            algorithm_id: 1,
            min_algorithm_id: 1,
            max_algorithm_id: 4,
            checksum: range(12, 16),
            covered: range(0, 12),
        };
        assert!(checksum_shape_valid(checksum));
        assert!(!checksum_shape_valid(ChecksumShapeEnvelope {
            checksum: range(15, 17),
            ..checksum
        }));
    }

    #[test]
    fn encoder_decoder_and_round_trip_shapes_compose() {
        let buffer = ByteBufferEnvelope {
            len: 16,
            capacity: 16,
            min_byte: 0,
            max_byte: 255,
        };
        let frame = FrameEnvelope {
            buffer_len: 16,
            header: range(0, 2),
            payload: range(2, 12),
            trailer: range(12, 16),
            has_trailer: true,
        };
        let encoder = EncoderShapeEnvelope {
            model_len: 3,
            max_model_len: 8,
            output_len: 16,
            max_output_len: 32,
            frame,
        };
        assert!(encoder_shape_valid(encoder));
        let decoder = DecoderShapeEnvelope {
            buffer,
            frame,
            length_prefix: LengthPrefixEnvelope {
                prefix: range(0, 2),
                remaining: range(2, 12),
                declared_len: 10,
            },
            schema: SchemaVersionEnvelope {
                schema_id: 1,
                expected_schema_id: 1,
                version: 1,
                min_supported_version: 1,
                max_supported_version: 2,
            },
            checksum: ChecksumShapeEnvelope {
                buffer_len: 16,
                algorithm_id: 1,
                min_algorithm_id: 1,
                max_algorithm_id: 1,
                checksum: range(12, 16),
                covered: range(0, 12),
            },
            require_checksum: true,
            checksum_present: true,
        };
        assert!(decoder_shape_valid(decoder));
        assert!(!decoder_shape_valid(DecoderShapeEnvelope {
            checksum_present: false,
            ..decoder
        }));
        assert!(bounded_round_trip_admitted(RoundTripEnvelope {
            model_admitted: true,
            encoder_shape_admitted: true,
            decoder_shape_admitted: true,
            same_schema: true,
            same_version: true,
            output_len: 16,
            max_output_len: 32
        }));
        assert!(!bounded_round_trip_admitted(RoundTripEnvelope {
            model_admitted: false,
            encoder_shape_admitted: true,
            decoder_shape_admitted: true,
            same_schema: true,
            same_version: true,
            output_len: 16,
            max_output_len: 32
        }));
    }
}
