#![allow(
    unknown_lints,
    non_trait_imports,
    unbounded_collection_growth,
    reason = "frame allocation occurs only after the declared length passes the configured hard bound; the fixed header constant keeps framing explicit"
)]

use std::io::Read;
use std::io::Write;

use crate::FRAME_HEADER_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFailure {
    Encoding,
    TooLarge,
    InvalidLength,
    InputEnded,
    ReadFailed,
    WriteFailed,
}

// r[impl lattice.workflow_interchange.transport_bounds]
pub fn encode_frame<T: serde::Serialize>(value: &T, maximum_frame_bytes: u32) -> Result<Vec<u8>, FrameFailure> {
    let body = serde_json::to_vec(value).map_err(|_| FrameFailure::Encoding)?;
    let body_length_bytes = u32::try_from(body.len()).map_err(|_| FrameFailure::TooLarge)?;
    if body_length_bytes == 0 || body_length_bytes > maximum_frame_bytes {
        return Err(FrameFailure::TooLarge);
    }
    let frame_length_bytes = FRAME_HEADER_BYTES.checked_add(body.len()).ok_or(FrameFailure::TooLarge)?;
    let mut frame = Vec::with_capacity(frame_length_bytes);
    frame.extend_from_slice(&body_length_bytes.to_be_bytes());
    frame.extend_from_slice(&body);
    debug_assert_eq!(frame.len(), frame_length_bytes);
    Ok(frame)
}

// r[impl lattice.workflow_interchange.transport_bounds]
pub fn read_frame<R: Read>(reader: &mut R, maximum_frame_bytes: u32) -> Result<Vec<u8>, FrameFailure> {
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    read_exact_bounded(reader, &mut header)?;
    let declared_length_bytes = u32::from_be_bytes(header);
    if declared_length_bytes == 0 || declared_length_bytes > maximum_frame_bytes {
        return Err(FrameFailure::TooLarge);
    }
    let body_length_bytes = usize::try_from(declared_length_bytes).map_err(|_| FrameFailure::InvalidLength)?;
    let mut body = vec![0_u8; body_length_bytes];
    read_exact_bounded(reader, &mut body)?;
    Ok(body)
}

// r[impl lattice.workflow_interchange.transport_bounds]
pub fn write_frame<W: Write>(writer: &mut W, frame: &[u8]) -> Result<(), FrameFailure> {
    let mut written = 0_usize;
    while written < frame.len() {
        let remaining = frame.get(written..).ok_or(FrameFailure::InvalidLength)?;
        let count = writer.write(remaining).map_err(|_| FrameFailure::WriteFailed)?;
        if count == 0 {
            return Err(FrameFailure::WriteFailed);
        }
        written = written.checked_add(count).ok_or(FrameFailure::InvalidLength)?;
    }
    writer.flush().map_err(|_| FrameFailure::WriteFailed)
}

fn read_exact_bounded<R: Read>(reader: &mut R, buffer: &mut [u8]) -> Result<(), FrameFailure> {
    let mut read = 0_usize;
    while read < buffer.len() {
        let remaining = buffer.get_mut(read..).ok_or(FrameFailure::InvalidLength)?;
        let count = reader.read(remaining).map_err(|_| FrameFailure::ReadFailed)?;
        if count == 0 {
            return Err(FrameFailure::InputEnded);
        }
        read = read.checked_add(count).ok_or(FrameFailure::InvalidLength)?;
    }
    Ok(())
}
