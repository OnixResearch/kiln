//! Finite GPU programming primitive admission checks.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuBlockDim {
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub max_threads_per_block: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuGridDim {
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub max_blocks: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuThreadIndex {
    pub thread_x: u16,
    pub thread_y: u16,
    pub thread_z: u16,
    pub block_x: u16,
    pub block_y: u16,
    pub block_z: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuMemoryAccess {
    pub base: u64,
    pub lane_count: u16,
    pub element_bytes: u16,
    pub segment_bytes: u32,
    pub buffer_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuSharedTile {
    pub rows: u16,
    pub cols: u16,
    pub element_bytes: u16,
    pub capacity_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuLaunchEnvelope {
    pub block: GpuBlockDim,
    pub grid: GpuGridDim,
    pub shared_tile: GpuSharedTile,
    pub max_shared_bytes_per_block: u64,
}

// r[depends gpu.block-dim]
pub open spec fn gpu_block_thread_count_spec(b: GpuBlockDim) -> nat {
    (b.x as nat) * (b.y as nat) * (b.z as nat)
}

// r[depends gpu.block-dim]
pub open spec fn gpu_block_dim_spec(b: GpuBlockDim) -> bool {
    b.x > 0 && b.y > 0 && b.z > 0 && b.max_threads_per_block > 0
        && gpu_block_thread_count_spec(b) <= b.max_threads_per_block as nat
}

// r[impl gpu.block-dim]
pub fn gpu_block_dim_valid(b: GpuBlockDim) -> (result: bool)
    ensures result == gpu_block_dim_spec(b)
{
    assert((b.x as u64) * (b.y as u64) <= u64::MAX) by(nonlinear_arith);
    assert((b.x as u64) * (b.y as u64) * (b.z as u64) <= u64::MAX) by(nonlinear_arith);
    b.x > 0 && b.y > 0 && b.z > 0 && b.max_threads_per_block > 0
        && (b.x as u64) * (b.y as u64) * (b.z as u64) <= b.max_threads_per_block as u64
}

// r[verify gpu.block-dim]
pub proof fn valid_gpu_block_dim_is_admitted(b: GpuBlockDim)
    requires gpu_block_dim_spec(b)
    ensures gpu_block_dim_spec(b)
{
}

// r[depends gpu.grid-dim]
pub open spec fn gpu_grid_block_count_spec(g: GpuGridDim) -> nat {
    (g.x as nat) * (g.y as nat) * (g.z as nat)
}

// r[depends gpu.grid-dim]
pub open spec fn gpu_grid_dim_spec(g: GpuGridDim) -> bool {
    g.x > 0 && g.y > 0 && g.z > 0 && g.max_blocks > 0
        && gpu_grid_block_count_spec(g) <= g.max_blocks as nat
}

// r[impl gpu.grid-dim]
pub fn gpu_grid_dim_valid(g: GpuGridDim) -> (result: bool)
    ensures result == gpu_grid_dim_spec(g)
{
    assert((g.x as u64) * (g.y as u64) <= u64::MAX) by(nonlinear_arith);
    assert((g.x as u64) * (g.y as u64) * (g.z as u64) <= u64::MAX) by(nonlinear_arith);
    g.x > 0 && g.y > 0 && g.z > 0 && g.max_blocks > 0
        && (g.x as u64) * (g.y as u64) * (g.z as u64) <= g.max_blocks
}

// r[verify gpu.grid-dim]
pub proof fn valid_gpu_grid_dim_is_admitted(g: GpuGridDim)
    requires gpu_grid_dim_spec(g)
    ensures gpu_grid_dim_spec(g)
{
}

// r[depends gpu.thread-index]
pub open spec fn gpu_thread_index_spec(i: GpuThreadIndex, b: GpuBlockDim, g: GpuGridDim) -> bool {
    gpu_block_dim_spec(b) && gpu_grid_dim_spec(g)
        && i.thread_x < b.x && i.thread_y < b.y && i.thread_z < b.z
        && i.block_x < g.x && i.block_y < g.y && i.block_z < g.z
}

// r[depends gpu.thread-index]
pub open spec fn gpu_local_thread_ordinal_spec(i: GpuThreadIndex, b: GpuBlockDim) -> nat {
    (i.thread_x as nat) + (b.x as nat) * ((i.thread_y as nat) + (b.y as nat) * (i.thread_z as nat))
}

// r[depends gpu.thread-index]
pub open spec fn gpu_block_ordinal_spec(i: GpuThreadIndex, g: GpuGridDim) -> nat {
    (i.block_x as nat) + (g.x as nat) * ((i.block_y as nat) + (g.y as nat) * (i.block_z as nat))
}

// r[impl gpu.thread-index]
pub fn gpu_thread_index_valid(i: GpuThreadIndex, b: GpuBlockDim, g: GpuGridDim) -> (result: bool)
    ensures result == gpu_thread_index_spec(i, b, g)
{
    gpu_block_dim_valid(b) && gpu_grid_dim_valid(g)
        && i.thread_x < b.x && i.thread_y < b.y && i.thread_z < b.z
        && i.block_x < g.x && i.block_y < g.y && i.block_z < g.z
}

// r[verify gpu.thread-index]
pub proof fn valid_gpu_thread_index_is_admitted(i: GpuThreadIndex, b: GpuBlockDim, g: GpuGridDim)
    requires gpu_thread_index_spec(i, b, g)
    ensures gpu_thread_index_spec(i, b, g)
{
}

// r[depends gpu.coalesced-access]
pub open spec fn gpu_memory_span_spec(a: GpuMemoryAccess) -> nat {
    (a.lane_count as nat) * (a.element_bytes as nat)
}

// r[depends gpu.coalesced-access]
pub open spec fn gpu_memory_access_spec(a: GpuMemoryAccess) -> bool {
    a.lane_count > 0 && a.element_bytes > 0 && a.segment_bytes > 0
        && gpu_memory_span_spec(a) <= a.segment_bytes as nat
        && a.base <= a.buffer_bytes
        && gpu_memory_span_spec(a) <= (a.buffer_bytes - a.base) as nat
}

// r[impl gpu.coalesced-access]
pub fn gpu_memory_access_valid(a: GpuMemoryAccess) -> (result: bool)
    ensures result == gpu_memory_access_spec(a)
{
    assert((a.lane_count as u64) * (a.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    a.lane_count > 0
        && a.element_bytes > 0
        && a.segment_bytes > 0
        && (a.lane_count as u64) * (a.element_bytes as u64) <= a.segment_bytes as u64
        && a.base <= a.buffer_bytes
        && (a.lane_count as u64) * (a.element_bytes as u64) <= a.buffer_bytes - a.base
}

// r[verify gpu.coalesced-access]
pub proof fn valid_gpu_memory_access_is_in_bounds(a: GpuMemoryAccess)
    requires gpu_memory_access_spec(a)
    ensures gpu_memory_access_spec(a), gpu_memory_span_spec(a) <= a.segment_bytes as nat
{}

// r[depends gpu.shared-tile]
pub open spec fn gpu_shared_tile_bytes_spec(t: GpuSharedTile) -> nat {
    (t.rows as nat) * (t.cols as nat) * (t.element_bytes as nat)
}

// r[depends gpu.shared-tile]
pub open spec fn gpu_shared_tile_spec(t: GpuSharedTile) -> bool {
    t.rows > 0 && t.cols > 0 && t.element_bytes > 0
        && gpu_shared_tile_bytes_spec(t) <= t.capacity_bytes as nat
}

// r[impl gpu.shared-tile]
pub fn gpu_shared_tile_valid(t: GpuSharedTile) -> (result: bool)
    ensures result == gpu_shared_tile_spec(t)
{
    assert((t.rows as u64) * (t.cols as u64) <= u64::MAX) by(nonlinear_arith);
    assert((t.rows as u64) * (t.cols as u64) * (t.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    t.rows > 0 && t.cols > 0 && t.element_bytes > 0
        && (t.rows as u64) * (t.cols as u64) * (t.element_bytes as u64) <= t.capacity_bytes
}

// r[verify gpu.shared-tile]
pub proof fn valid_gpu_shared_tile_fits_capacity(t: GpuSharedTile)
    requires gpu_shared_tile_spec(t)
    ensures gpu_shared_tile_bytes_spec(t) <= t.capacity_bytes as nat
{}

// r[depends gpu.launch-envelope]
pub open spec fn gpu_launch_envelope_spec(e: GpuLaunchEnvelope) -> bool {
    gpu_block_dim_spec(e.block) && gpu_grid_dim_spec(e.grid) && gpu_shared_tile_spec(e.shared_tile)
        && e.shared_tile.capacity_bytes <= e.max_shared_bytes_per_block
}

// r[impl gpu.launch-envelope]
pub fn gpu_launch_envelope_valid(e: GpuLaunchEnvelope) -> (result: bool)
    ensures result == gpu_launch_envelope_spec(e)
{
    gpu_block_dim_valid(e.block) && gpu_grid_dim_valid(e.grid) && gpu_shared_tile_valid(e.shared_tile)
        && e.shared_tile.capacity_bytes <= e.max_shared_bytes_per_block
}

// r[verify gpu.launch-envelope]
pub proof fn valid_gpu_launch_envelope_components_are_valid(e: GpuLaunchEnvelope)
    requires gpu_launch_envelope_spec(e)
    ensures gpu_block_dim_spec(e.block), gpu_grid_dim_spec(e.grid), gpu_shared_tile_spec(e.shared_tile)
{}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuPitchedBuffer {
    pub width: u16,
    pub height: u16,
    pub element_bytes: u16,
    pub pitch_bytes: u32,
    pub buffer_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuTensor3D {
    pub width: u16,
    pub height: u16,
    pub depth: u16,
    pub element_bytes: u16,
    pub buffer_bytes: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuTensorIndex3D {
    pub x: u16,
    pub y: u16,
    pub z: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuTileProjection {
    pub origin_x: u16,
    pub origin_y: u16,
    pub rows: u16,
    pub cols: u16,
}

// r[depends gpu.pitched-buffer]
pub open spec fn gpu_pitched_row_bytes_spec(b: GpuPitchedBuffer) -> nat {
    (b.width as nat) * (b.element_bytes as nat)
}

// r[depends gpu.pitched-buffer]
pub open spec fn gpu_pitched_buffer_spec(b: GpuPitchedBuffer) -> bool {
    b.width > 0 && b.height > 0 && b.element_bytes > 0
        && gpu_pitched_row_bytes_spec(b) <= b.pitch_bytes as nat
        && (b.pitch_bytes as nat) * (b.height as nat) <= b.buffer_bytes as nat
}

// r[impl gpu.pitched-buffer]
pub fn gpu_pitched_buffer_valid(b: GpuPitchedBuffer) -> (result: bool)
    ensures result == gpu_pitched_buffer_spec(b)
{
    assert((b.width as u64) * (b.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    assert((b.pitch_bytes as u64) * (b.height as u64) <= u64::MAX) by(nonlinear_arith);
    b.width > 0
        && b.height > 0
        && b.element_bytes > 0
        && (b.width as u64) * (b.element_bytes as u64) <= b.pitch_bytes as u64
        && (b.pitch_bytes as u64) * (b.height as u64) <= b.buffer_bytes
}

// r[verify gpu.pitched-buffer]
pub proof fn valid_gpu_pitched_buffer_is_admitted(b: GpuPitchedBuffer)
    requires gpu_pitched_buffer_spec(b)
    ensures gpu_pitched_buffer_spec(b), gpu_pitched_row_bytes_spec(b) <= b.pitch_bytes as nat
{}

// r[depends gpu.pitched-address]
pub open spec fn gpu_pitched_offset_spec(b: GpuPitchedBuffer, x: u16, y: u16) -> nat {
    (y as nat) * (b.pitch_bytes as nat) + (x as nat) * (b.element_bytes as nat)
}

// r[depends gpu.pitched-address]
pub open spec fn gpu_pitched_address_spec(b: GpuPitchedBuffer, x: u16, y: u16) -> bool {
    gpu_pitched_buffer_spec(b) && x < b.width && y < b.height
        && (b.element_bytes as nat) <= b.buffer_bytes as nat
        && gpu_pitched_offset_spec(b, x, y) + (b.element_bytes as nat) <= b.buffer_bytes as nat
}

// r[impl gpu.pitched-address]
pub fn gpu_pitched_address_valid(b: GpuPitchedBuffer, x: u16, y: u16) -> (result: bool)
    ensures result == gpu_pitched_address_spec(b, x, y)
{
    assert((y as u64) * (b.pitch_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    assert((x as u64) * (b.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    assert((y as u64) * (b.pitch_bytes as u64) + (x as u64) * (b.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    gpu_pitched_buffer_valid(b)
        && x < b.width
        && y < b.height
        && (b.element_bytes as u64) <= b.buffer_bytes
        && (y as u64) * (b.pitch_bytes as u64) + (x as u64) * (b.element_bytes as u64)
            <= b.buffer_bytes - b.element_bytes as u64
}

// r[verify gpu.pitched-address]
pub proof fn valid_gpu_pitched_address_is_in_bounds(b: GpuPitchedBuffer, x: u16, y: u16)
    requires gpu_pitched_address_spec(b, x, y)
    ensures gpu_pitched_offset_spec(b, x, y) + (b.element_bytes as nat) <= b.buffer_bytes as nat
{}

// r[depends gpu.tensor3d-layout]
pub open spec fn gpu_tensor3d_element_count_spec(t: GpuTensor3D) -> nat {
    (t.width as nat) * (t.height as nat) * (t.depth as nat)
}

// r[depends gpu.tensor3d-layout]
pub open spec fn gpu_tensor3d_bytes_spec(t: GpuTensor3D) -> nat {
    gpu_tensor3d_element_count_spec(t) * (t.element_bytes as nat)
}

// r[depends gpu.tensor3d-layout]
pub open spec fn gpu_tensor3d_spec(t: GpuTensor3D) -> bool {
    t.width > 0 && t.height > 0 && t.depth > 0 && t.element_bytes > 0
        && gpu_tensor3d_bytes_spec(t) <= t.buffer_bytes as nat
}

// r[impl gpu.tensor3d-layout]
pub fn gpu_tensor3d_valid(t: GpuTensor3D) -> (result: bool)
    ensures result == gpu_tensor3d_spec(t)
{
    assert((t.width as u64) * (t.height as u64) <= u64::MAX) by(nonlinear_arith);
    assert((t.width as u64) * (t.height as u64) * (t.depth as u64) <= u64::MAX) by(nonlinear_arith);
    assert((t.width as u64) * (t.height as u64) * (t.depth as u64) * (t.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    t.width > 0
        && t.height > 0
        && t.depth > 0
        && t.element_bytes > 0
        && (t.width as u64) * (t.height as u64) * (t.depth as u64) * (t.element_bytes as u64)
            <= t.buffer_bytes
}

// r[verify gpu.tensor3d-layout]
pub proof fn valid_gpu_tensor3d_is_admitted(t: GpuTensor3D)
    requires gpu_tensor3d_spec(t)
    ensures gpu_tensor3d_spec(t), gpu_tensor3d_bytes_spec(t) <= t.buffer_bytes as nat
{}

// r[depends gpu.tensor3d-index]
pub open spec fn gpu_tensor3d_index_spec(t: GpuTensor3D, i: GpuTensorIndex3D) -> bool {
    gpu_tensor3d_spec(t) && i.x < t.width && i.y < t.height && i.z < t.depth
}

// r[depends gpu.tensor3d-index]
pub open spec fn gpu_tensor3d_offset_spec(t: GpuTensor3D, i: GpuTensorIndex3D) -> nat {
    (((i.z as nat) * (t.height as nat) + (i.y as nat)) * (t.width as nat) + (i.x as nat))
        * (t.element_bytes as nat)
}

// r[impl gpu.tensor3d-index]
pub fn gpu_tensor3d_index_valid(t: GpuTensor3D, i: GpuTensorIndex3D) -> (result: bool)
    ensures result == gpu_tensor3d_index_spec(t, i)
{
    gpu_tensor3d_valid(t) && i.x < t.width && i.y < t.height && i.z < t.depth
}

// r[verify gpu.tensor3d-index]
pub proof fn valid_gpu_tensor3d_index_is_admitted(t: GpuTensor3D, i: GpuTensorIndex3D)
    requires gpu_tensor3d_index_spec(t, i)
    ensures gpu_tensor3d_index_spec(t, i)
{}

// r[depends gpu.tile-projection]
pub open spec fn gpu_tile_projection_spec(b: GpuPitchedBuffer, tile: GpuTileProjection) -> bool {
    gpu_pitched_buffer_spec(b)
        && tile.rows > 0
        && tile.cols > 0
        && (tile.origin_x as nat) + (tile.cols as nat) <= b.width as nat
        && (tile.origin_y as nat) + (tile.rows as nat) <= b.height as nat
}

// r[impl gpu.tile-projection]
pub fn gpu_tile_projection_valid(b: GpuPitchedBuffer, tile: GpuTileProjection) -> (result: bool)
    ensures result == gpu_tile_projection_spec(b, tile)
{
    gpu_pitched_buffer_valid(b)
        && tile.rows > 0
        && tile.cols > 0
        && (tile.origin_x as u64) + (tile.cols as u64) <= b.width as u64
        && (tile.origin_y as u64) + (tile.rows as u64) <= b.height as u64
}

// r[verify gpu.tile-projection]
pub proof fn valid_gpu_tile_projection_is_admitted(b: GpuPitchedBuffer, tile: GpuTileProjection)
    requires gpu_tile_projection_spec(b, tile)
    ensures gpu_tile_projection_spec(b, tile)
{}


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuElementwiseProgram {
    pub elements: u64,
    pub threads: u16,
    pub elements_per_thread: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuGridStrideLoop {
    pub len: u64,
    pub start: u64,
    pub stride: u16,
    pub iterations: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuTiledKernel2D {
    pub width: u16,
    pub height: u16,
    pub tile_width: u16,
    pub tile_height: u16,
    pub grid_x: u16,
    pub grid_y: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuBlockReductionProgram {
    pub input_len: u64,
    pub block_threads: u16,
    pub elements_per_thread: u16,
    pub blocks: u16,
    pub element_bytes: u16,
    pub shared_capacity_bytes: u64,
}

// r[depends gpu.elementwise-program]
pub open spec fn gpu_elementwise_program_capacity_spec(p: GpuElementwiseProgram) -> nat {
    (p.threads as nat) * (p.elements_per_thread as nat)
}

// r[depends gpu.elementwise-program]
pub open spec fn gpu_elementwise_program_spec(p: GpuElementwiseProgram) -> bool {
    p.elements > 0 && p.threads > 0 && p.elements_per_thread > 0
        && p.elements as nat <= gpu_elementwise_program_capacity_spec(p)
}

// r[impl gpu.elementwise-program]
pub fn gpu_elementwise_program_valid(p: GpuElementwiseProgram) -> (result: bool)
    ensures result == gpu_elementwise_program_spec(p)
{
    assert((p.threads as u64) * (p.elements_per_thread as u64) <= u64::MAX) by(nonlinear_arith);
    p.elements > 0
        && p.threads > 0
        && p.elements_per_thread > 0
        && p.elements <= (p.threads as u64) * (p.elements_per_thread as u64)
}

// r[verify gpu.elementwise-program]
pub proof fn valid_gpu_elementwise_program_covers_elements(p: GpuElementwiseProgram)
    requires gpu_elementwise_program_spec(p)
    ensures p.elements as nat <= gpu_elementwise_program_capacity_spec(p)
{}

// r[depends gpu.grid-stride-loop]
pub open spec fn gpu_grid_stride_last_index_spec(l: GpuGridStrideLoop) -> int {
    (l.start as int) + ((l.iterations as int) - 1int) * (l.stride as int)
}

// r[depends gpu.grid-stride-loop]
pub open spec fn gpu_grid_stride_loop_spec(l: GpuGridStrideLoop) -> bool {
    l.len > 0 && l.stride > 0 && l.iterations > 0 && l.start < l.len
        && gpu_grid_stride_last_index_spec(l) < l.len as int
}

// r[impl gpu.grid-stride-loop]
pub fn gpu_grid_stride_loop_valid(l: GpuGridStrideLoop) -> (result: bool)
    ensures result == gpu_grid_stride_loop_spec(l)
{
    if !(l.len > 0 && l.stride > 0 && l.iterations > 0 && l.start < l.len) {
        false
    } else {
        assert(((l.iterations as u64) - 1u64) * (l.stride as u64) <= u64::MAX) by(nonlinear_arith);
        ((l.iterations as u64) - 1u64) * (l.stride as u64) <= (l.len - 1u64) - l.start
    }
}

// r[verify gpu.grid-stride-loop]
pub proof fn valid_gpu_grid_stride_loop_stays_in_bounds(l: GpuGridStrideLoop)
    requires gpu_grid_stride_loop_spec(l)
    ensures gpu_grid_stride_last_index_spec(l) < l.len as int
{}

// r[depends gpu.tiled-kernel2d]
pub open spec fn gpu_tiled_kernel2d_covers_spec(k: GpuTiledKernel2D) -> bool {
    (k.grid_x as nat) * (k.tile_width as nat) >= k.width as nat
        && (k.grid_y as nat) * (k.tile_height as nat) >= k.height as nat
}

// r[depends gpu.tiled-kernel2d]
pub open spec fn gpu_tiled_kernel2d_spec(k: GpuTiledKernel2D) -> bool {
    k.width > 0 && k.height > 0 && k.tile_width > 0 && k.tile_height > 0
        && k.grid_x > 0 && k.grid_y > 0 && gpu_tiled_kernel2d_covers_spec(k)
}

// r[impl gpu.tiled-kernel2d]
pub fn gpu_tiled_kernel2d_valid(k: GpuTiledKernel2D) -> (result: bool)
    ensures result == gpu_tiled_kernel2d_spec(k)
{
    assert((k.grid_x as u64) * (k.tile_width as u64) <= u64::MAX) by(nonlinear_arith);
    assert((k.grid_y as u64) * (k.tile_height as u64) <= u64::MAX) by(nonlinear_arith);
    k.width > 0
        && k.height > 0
        && k.tile_width > 0
        && k.tile_height > 0
        && k.grid_x > 0
        && k.grid_y > 0
        && (k.grid_x as u64) * (k.tile_width as u64) >= k.width as u64
        && (k.grid_y as u64) * (k.tile_height as u64) >= k.height as u64
}

// r[verify gpu.tiled-kernel2d]
pub proof fn valid_gpu_tiled_kernel2d_covers_output(k: GpuTiledKernel2D)
    requires gpu_tiled_kernel2d_spec(k)
    ensures gpu_tiled_kernel2d_covers_spec(k)
{}

// r[depends gpu.block-reduction-program]
pub open spec fn gpu_block_reduction_capacity_spec(p: GpuBlockReductionProgram) -> nat {
    (p.block_threads as nat) * (p.elements_per_thread as nat) * (p.blocks as nat)
}

// r[depends gpu.block-reduction-program]
pub open spec fn gpu_block_reduction_shared_bytes_spec(p: GpuBlockReductionProgram) -> nat {
    (p.block_threads as nat) * (p.element_bytes as nat)
}

// r[depends gpu.block-reduction-program]
pub open spec fn gpu_block_reduction_program_spec(p: GpuBlockReductionProgram) -> bool {
    p.input_len > 0 && p.block_threads > 0 && p.elements_per_thread > 0 && p.blocks > 0
        && p.element_bytes > 0
        && p.input_len as nat <= gpu_block_reduction_capacity_spec(p)
        && gpu_block_reduction_shared_bytes_spec(p) <= p.shared_capacity_bytes as nat
}

// r[impl gpu.block-reduction-program]
pub fn gpu_block_reduction_program_valid(p: GpuBlockReductionProgram) -> (result: bool)
    ensures result == gpu_block_reduction_program_spec(p)
{
    assert((p.block_threads as u64) * (p.elements_per_thread as u64) <= u64::MAX) by(nonlinear_arith);
    assert((p.block_threads as u64) * (p.elements_per_thread as u64) * (p.blocks as u64) <= u64::MAX) by(nonlinear_arith);
    assert((p.block_threads as u64) * (p.element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    p.input_len > 0
        && p.block_threads > 0
        && p.elements_per_thread > 0
        && p.blocks > 0
        && p.element_bytes > 0
        && p.input_len
            <= (p.block_threads as u64) * (p.elements_per_thread as u64) * (p.blocks as u64)
        && (p.block_threads as u64) * (p.element_bytes as u64) <= p.shared_capacity_bytes
}

// r[verify gpu.block-reduction-program]
pub proof fn valid_gpu_block_reduction_program_has_capacity(p: GpuBlockReductionProgram)
    requires gpu_block_reduction_program_spec(p)
    ensures
        p.input_len as nat <= gpu_block_reduction_capacity_spec(p),
        gpu_block_reduction_shared_bytes_spec(p) <= p.shared_capacity_bytes as nat
{}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuReductionStage {
    pub input_count: u64,
    pub output_count: u64,
    pub active_lanes: u16,
    pub max_lanes: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuReductionTreeProgram {
    pub declared_outputs: u64,
    pub block_program: GpuBlockReductionProgram,
}

// r[depends domain-primitives.gpu.reduction-tree-program]
pub open spec fn gpu_reduction_stage_spec(s: GpuReductionStage) -> bool {
    s.input_count > 0
        && s.output_count > 0
        && s.output_count <= s.input_count
        && s.active_lanes > 0
        && s.max_lanes > 0
        && s.active_lanes <= s.max_lanes
        && s.active_lanes as nat <= s.input_count as nat
}

// r[depends domain-primitives.gpu.reduction-tree-program]
pub open spec fn gpu_reduction_tree_from_spec(
    stages: Seq<GpuReductionStage>,
    index: nat,
    current_count: u64,
    declared_outputs: u64,
) -> bool
    decreases stages.len() - index
{
    if index >= stages.len() {
        declared_outputs > 0 && current_count == declared_outputs && index == stages.len()
    } else {
        gpu_reduction_stage_spec(stages[index as int])
            && stages[index as int].input_count == current_count
            && gpu_reduction_tree_from_spec(
                stages,
                index + 1,
                stages[index as int].output_count,
                declared_outputs,
            )
    }
}

// r[depends domain-primitives.gpu.reduction-tree-program]
pub open spec fn gpu_reduction_tree_program_spec(
    p: GpuReductionTreeProgram,
    stages: Seq<GpuReductionStage>,
) -> bool {
    gpu_block_reduction_program_spec(p.block_program)
        && stages.len() > 0
        && gpu_reduction_tree_from_spec(stages, 0, p.block_program.input_len, p.declared_outputs)
}

// r[impl domain-primitives.gpu.reduction-tree-program]
pub fn gpu_reduction_stage_valid(s: GpuReductionStage) -> (result: bool)
    ensures result == gpu_reduction_stage_spec(s)
{
    s.input_count > 0
        && s.output_count > 0
        && s.output_count <= s.input_count
        && s.active_lanes > 0
        && s.max_lanes > 0
        && s.active_lanes <= s.max_lanes
        && (s.active_lanes as u64) <= s.input_count
}

fn gpu_reduction_tree_from_valid(
    stages: &Vec<GpuReductionStage>,
    index: usize,
    current_count: u64,
    declared_outputs: u64,
) -> (result: bool)
    requires index <= stages@.len()
    ensures
        result == gpu_reduction_tree_from_spec(
            stages@,
            index as nat,
            current_count,
            declared_outputs,
        )
    decreases stages@.len() - index
{
    if index == stages.len() {
        declared_outputs > 0 && current_count == declared_outputs
    } else {
        let stage = stages[index];
        gpu_reduction_stage_valid(stage)
            && stage.input_count == current_count
            && gpu_reduction_tree_from_valid(stages, index + 1, stage.output_count, declared_outputs)
    }
}

// r[impl domain-primitives.gpu.reduction-tree-program]
pub fn gpu_reduction_tree_program_valid(
    p: GpuReductionTreeProgram,
    stages: &Vec<GpuReductionStage>,
) -> (result: bool)
    ensures result == gpu_reduction_tree_program_spec(p, stages@)
{
    gpu_block_reduction_program_valid(p.block_program)
        && stages.len() > 0
        && gpu_reduction_tree_from_valid(stages, 0, p.block_program.input_len, p.declared_outputs)
}

// r[verify domain-primitives.gpu.reduction-tree-program]
pub proof fn valid_gpu_reduction_stage_is_non_expanding(s: GpuReductionStage)
    requires gpu_reduction_stage_spec(s)
    ensures
        s.output_count > 0,
        s.output_count <= s.input_count,
        s.active_lanes > 0,
        s.active_lanes <= s.max_lanes
{}

// r[verify domain-primitives.gpu.reduction-tree-program]
pub proof fn reduction_tree_from_declared_outputs_positive(
    stages: Seq<GpuReductionStage>,
    index: nat,
    current_count: u64,
    declared_outputs: u64,
)
    requires
        index <= stages.len(),
        gpu_reduction_tree_from_spec(stages, index, current_count, declared_outputs)
    ensures declared_outputs > 0
    decreases stages.len() - index
{
    if index < stages.len() {
        reduction_tree_from_declared_outputs_positive(
            stages,
            index + 1,
            stages[index as int].output_count,
            declared_outputs,
        );
    }
}

// r[verify domain-primitives.gpu.reduction-tree-program]
pub proof fn valid_gpu_reduction_tree_reaches_declared_outputs(
    p: GpuReductionTreeProgram,
    stages: Seq<GpuReductionStage>,
)
    requires gpu_reduction_tree_program_spec(p, stages)
    ensures
        stages.len() > 0,
        p.declared_outputs > 0,
        gpu_block_reduction_program_spec(p.block_program)
{
    reduction_tree_from_declared_outputs_positive(stages, 0, p.block_program.input_len, p.declared_outputs);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuStencilDomain2D {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuStencilWindow2D {
    pub domain: GpuStencilDomain2D,
    pub center_x: u16,
    pub center_y: u16,
    pub radius_x: u16,
    pub radius_y: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuStencilRead2D {
    pub x: u16,
    pub y: u16,
}

// r[depends domain-primitives.gpu.stencil-boundary-program]
pub open spec fn gpu_stencil_domain2d_spec(d: GpuStencilDomain2D) -> bool {
    d.width > 0 && d.height > 0
}

// r[depends domain-primitives.gpu.stencil-boundary-program]
pub open spec fn gpu_stencil_point_in_domain_spec(d: GpuStencilDomain2D, x: u16, y: u16) -> bool {
    gpu_stencil_domain2d_spec(d) && x < d.width && y < d.height
}

// r[depends domain-primitives.gpu.stencil-boundary-program]
pub open spec fn gpu_stencil_full_window_spec(w: GpuStencilWindow2D) -> bool {
    gpu_stencil_point_in_domain_spec(w.domain, w.center_x, w.center_y)
        && w.center_x >= w.radius_x
        && w.center_y >= w.radius_y
        && (w.center_x as u64) + (w.radius_x as u64) < w.domain.width as u64
        && (w.center_y as u64) + (w.radius_y as u64) < w.domain.height as u64
}

// r[depends domain-primitives.gpu.stencil-boundary-program]
pub open spec fn gpu_stencil_guarded_window_spec(w: GpuStencilWindow2D) -> bool {
    gpu_stencil_point_in_domain_spec(w.domain, w.center_x, w.center_y)
}

// r[depends domain-primitives.gpu.stencil-boundary-program]
pub open spec fn gpu_stencil_guarded_read_spec(w: GpuStencilWindow2D, read: GpuStencilRead2D) -> bool {
    gpu_stencil_guarded_window_spec(w)
        && gpu_stencil_point_in_domain_spec(w.domain, read.x, read.y)
        && read.x as u64 <= (w.center_x as u64) + (w.radius_x as u64)
        && w.center_x as u64 <= (read.x as u64) + (w.radius_x as u64)
        && read.y as u64 <= (w.center_y as u64) + (w.radius_y as u64)
        && w.center_y as u64 <= (read.y as u64) + (w.radius_y as u64)
}

// r[impl domain-primitives.gpu.stencil-boundary-program]
pub fn gpu_stencil_domain2d_valid(d: GpuStencilDomain2D) -> (result: bool)
    ensures result == gpu_stencil_domain2d_spec(d)
{
    d.width > 0 && d.height > 0
}

// r[impl domain-primitives.gpu.stencil-boundary-program]
pub fn gpu_stencil_point_in_domain_valid(d: GpuStencilDomain2D, x: u16, y: u16) -> (result: bool)
    ensures result == gpu_stencil_point_in_domain_spec(d, x, y)
{
    gpu_stencil_domain2d_valid(d) && x < d.width && y < d.height
}

// r[impl domain-primitives.gpu.stencil-boundary-program]
pub fn gpu_stencil_full_window_valid(w: GpuStencilWindow2D) -> (result: bool)
    ensures result == gpu_stencil_full_window_spec(w)
{
    gpu_stencil_point_in_domain_valid(w.domain, w.center_x, w.center_y)
        && w.center_x >= w.radius_x
        && w.center_y >= w.radius_y
        && (w.center_x as u64) + (w.radius_x as u64) < w.domain.width as u64
        && (w.center_y as u64) + (w.radius_y as u64) < w.domain.height as u64
}

// r[impl domain-primitives.gpu.stencil-boundary-program]
pub fn gpu_stencil_guarded_window_valid(w: GpuStencilWindow2D) -> (result: bool)
    ensures result == gpu_stencil_guarded_window_spec(w)
{
    gpu_stencil_point_in_domain_valid(w.domain, w.center_x, w.center_y)
}

// r[impl domain-primitives.gpu.stencil-boundary-program]
pub fn gpu_stencil_guarded_read_valid(w: GpuStencilWindow2D, read: GpuStencilRead2D) -> (result: bool)
    ensures result == gpu_stencil_guarded_read_spec(w, read)
{
    gpu_stencil_guarded_window_valid(w)
        && gpu_stencil_point_in_domain_valid(w.domain, read.x, read.y)
        && read.x as u64 <= (w.center_x as u64) + (w.radius_x as u64)
        && w.center_x as u64 <= (read.x as u64) + (w.radius_x as u64)
        && read.y as u64 <= (w.center_y as u64) + (w.radius_y as u64)
        && w.center_y as u64 <= (read.y as u64) + (w.radius_y as u64)
}

// r[verify domain-primitives.gpu.stencil-boundary-program]
pub proof fn valid_gpu_stencil_full_window_reads_are_in_bounds(w: GpuStencilWindow2D)
    requires gpu_stencil_full_window_spec(w)
    ensures
        gpu_stencil_point_in_domain_spec(w.domain, w.center_x, w.center_y),
        w.center_x >= w.radius_x,
        w.center_y >= w.radius_y,
        (w.center_x as u64) + (w.radius_x as u64) < w.domain.width as u64,
        (w.center_y as u64) + (w.radius_y as u64) < w.domain.height as u64,
{}

// r[verify domain-primitives.gpu.stencil-boundary-program]
pub proof fn valid_gpu_stencil_guarded_read_is_in_bounds(w: GpuStencilWindow2D, read: GpuStencilRead2D)
    requires gpu_stencil_guarded_read_spec(w, read)
    ensures gpu_stencil_point_in_domain_spec(w.domain, read.x, read.y)
{}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GpuOutputWriteRegion {
    pub base: u64,
    pub span_bytes: u16,
    pub buffer_bytes: u64,
    pub logical_index: u64,
}

// r[depends domain-primitives.gpu.write-disjoint-program]
pub open spec fn gpu_output_write_region_end_spec(r: GpuOutputWriteRegion) -> nat {
    (r.base as nat) + (r.span_bytes as nat)
}

// r[depends domain-primitives.gpu.write-disjoint-program]
pub open spec fn gpu_output_write_region_spec(r: GpuOutputWriteRegion) -> bool {
    r.span_bytes > 0
        && r.base <= r.buffer_bytes
        && r.span_bytes as nat <= (r.buffer_bytes - r.base) as nat
}

// r[depends domain-primitives.gpu.write-disjoint-program]
pub open spec fn gpu_write_regions_disjoint_or_same_spec(a: GpuOutputWriteRegion, b: GpuOutputWriteRegion) -> bool {
    gpu_output_write_region_spec(a)
        && gpu_output_write_region_spec(b)
        && a.buffer_bytes == b.buffer_bytes
        && (a.logical_index == b.logical_index
            || gpu_output_write_region_end_spec(a) <= b.base as nat
            || gpu_output_write_region_end_spec(b) <= a.base as nat)
}

// r[depends domain-primitives.gpu.write-disjoint-program]
pub open spec fn gpu_elementwise_output_write_spec(
    p: GpuElementwiseProgram,
    logical_index: u32,
    element_bytes: u16,
    buffer_bytes: u64,
) -> bool {
    gpu_elementwise_program_spec(p)
        && (logical_index as u64) < p.elements
        && element_bytes > 0
        && (logical_index as nat) * (element_bytes as nat) <= buffer_bytes as nat
        && element_bytes as nat <= (buffer_bytes as nat) - ((logical_index as nat) * (element_bytes as nat))
}

// r[depends domain-primitives.gpu.write-disjoint-program]
pub open spec fn gpu_tiled_output_write_spec(
    k: GpuTiledKernel2D,
    output: GpuPitchedBuffer,
    x: u16,
    y: u16,
) -> bool {
    gpu_tiled_kernel2d_spec(k)
        && gpu_pitched_address_spec(output, x, y)
        && x < k.width
        && y < k.height
}

// r[impl domain-primitives.gpu.write-disjoint-program]
pub fn gpu_output_write_region_valid(r: GpuOutputWriteRegion) -> (result: bool)
    ensures result == gpu_output_write_region_spec(r)
{
    r.span_bytes > 0
        && r.base <= r.buffer_bytes
        && r.span_bytes as u64 <= r.buffer_bytes - r.base
}

// r[impl domain-primitives.gpu.write-disjoint-program]
pub fn gpu_write_regions_disjoint_or_same_valid(
    a: GpuOutputWriteRegion,
    b: GpuOutputWriteRegion,
) -> (result: bool)
    ensures result == gpu_write_regions_disjoint_or_same_spec(a, b)
{
    gpu_output_write_region_valid(a)
        && gpu_output_write_region_valid(b)
        && a.buffer_bytes == b.buffer_bytes
        && (a.logical_index == b.logical_index
            || a.base + a.span_bytes as u64 <= b.base
            || b.base + b.span_bytes as u64 <= a.base)
}

// r[impl domain-primitives.gpu.write-disjoint-program]
pub fn gpu_elementwise_output_write_valid(
    p: GpuElementwiseProgram,
    logical_index: u32,
    element_bytes: u16,
    buffer_bytes: u64,
) -> (result: bool)
    ensures result == gpu_elementwise_output_write_spec(p, logical_index, element_bytes, buffer_bytes)
{
    assert((logical_index as u64) * (element_bytes as u64) <= u64::MAX) by(nonlinear_arith);
    gpu_elementwise_program_valid(p)
        && (logical_index as u64) < p.elements
        && element_bytes > 0
        && (logical_index as u64) * (element_bytes as u64) <= buffer_bytes
        && element_bytes as u64 <= buffer_bytes - ((logical_index as u64) * (element_bytes as u64))
}

// r[impl domain-primitives.gpu.write-disjoint-program]
pub fn gpu_tiled_output_write_valid(
    k: GpuTiledKernel2D,
    output: GpuPitchedBuffer,
    x: u16,
    y: u16,
) -> (result: bool)
    ensures result == gpu_tiled_output_write_spec(k, output, x, y)
{
    gpu_tiled_kernel2d_valid(k)
        && gpu_pitched_address_valid(output, x, y)
        && x < k.width
        && y < k.height
}

// r[verify domain-primitives.gpu.write-disjoint-program]
pub proof fn valid_gpu_output_write_region_stays_in_buffer(r: GpuOutputWriteRegion)
    requires gpu_output_write_region_spec(r)
    ensures gpu_output_write_region_end_spec(r) <= r.buffer_bytes as nat
{}

// r[verify domain-primitives.gpu.write-disjoint-program]
pub proof fn valid_gpu_write_regions_distinct_are_disjoint(a: GpuOutputWriteRegion, b: GpuOutputWriteRegion)
    requires
        gpu_write_regions_disjoint_or_same_spec(a, b),
        a.logical_index != b.logical_index,
    ensures
        gpu_output_write_region_end_spec(a) <= b.base as nat
            || gpu_output_write_region_end_spec(b) <= a.base as nat,
{}

// r[verify domain-primitives.gpu.write-disjoint-program]
pub proof fn valid_gpu_tiled_output_write_inside_buffer(
    k: GpuTiledKernel2D,
    output: GpuPitchedBuffer,
    x: u16,
    y: u16,
)
    requires gpu_tiled_output_write_spec(k, output, x, y)
    ensures gpu_pitched_address_spec(output, x, y)
{}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_block() -> GpuBlockDim {
        GpuBlockDim {
            x: 16,
            y: 8,
            z: 1,
            max_threads_per_block: 256,
        }
    }

    fn valid_grid() -> GpuGridDim {
        GpuGridDim {
            x: 8,
            y: 4,
            z: 2,
            max_blocks: 64,
        }
    }

    #[test]
    fn block_dim_accepts_positive_product_under_limit() {
        assert!(gpu_block_dim_valid(valid_block()));
    }

    #[test]
    fn block_dim_rejects_zero_or_too_many_threads() {
        assert!(!gpu_block_dim_valid(GpuBlockDim {
            x: 0,
            y: 8,
            z: 1,
            max_threads_per_block: 256
        }));
        assert!(!gpu_block_dim_valid(GpuBlockDim {
            x: 32,
            y: 32,
            z: 2,
            max_threads_per_block: 1024
        }));
    }

    #[test]
    fn thread_index_checks_block_and_thread_bounds() {
        let i = GpuThreadIndex {
            thread_x: 15,
            thread_y: 7,
            thread_z: 0,
            block_x: 7,
            block_y: 3,
            block_z: 1,
        };
        assert!(gpu_thread_index_valid(i, valid_block(), valid_grid()));
        assert!(!gpu_thread_index_valid(GpuThreadIndex { thread_x: 16, ..i }, valid_block(), valid_grid()));
        assert!(!gpu_thread_index_valid(GpuThreadIndex { block_z: 2, ..i }, valid_block(), valid_grid()));
    }

    #[test]
    fn memory_access_requires_segment_and_buffer_fit() {
        assert!(gpu_memory_access_valid(GpuMemoryAccess {
            base: 128,
            lane_count: 32,
            element_bytes: 4,
            segment_bytes: 128,
            buffer_bytes: 4096
        }));
        assert!(!gpu_memory_access_valid(GpuMemoryAccess {
            base: 128,
            lane_count: 32,
            element_bytes: 8,
            segment_bytes: 128,
            buffer_bytes: 4096
        }));
        assert!(!gpu_memory_access_valid(GpuMemoryAccess {
            base: 4000,
            lane_count: 32,
            element_bytes: 4,
            segment_bytes: 128,
            buffer_bytes: 4096
        }));
    }

    #[test]
    fn launch_envelope_requires_shared_tile_within_launch_limit() {
        let tile = GpuSharedTile {
            rows: 16,
            cols: 16,
            element_bytes: 4,
            capacity_bytes: 1024,
        };
        assert!(gpu_launch_envelope_valid(GpuLaunchEnvelope {
            block: valid_block(),
            grid: valid_grid(),
            shared_tile: tile,
            max_shared_bytes_per_block: 2048
        }));
        assert!(!gpu_launch_envelope_valid(GpuLaunchEnvelope {
            block: valid_block(),
            grid: valid_grid(),
            shared_tile: tile,
            max_shared_bytes_per_block: 512
        }));
    }
    fn valid_pitched_buffer() -> GpuPitchedBuffer {
        GpuPitchedBuffer {
            width: 64,
            height: 32,
            element_bytes: 4,
            pitch_bytes: 256,
            buffer_bytes: 8192,
        }
    }

    #[test]
    fn pitched_buffer_requires_pitch_and_backing_capacity() {
        assert!(gpu_pitched_buffer_valid(valid_pitched_buffer()));
        assert!(!gpu_pitched_buffer_valid(GpuPitchedBuffer {
            pitch_bytes: 128,
            ..valid_pitched_buffer()
        }));
        assert!(!gpu_pitched_buffer_valid(GpuPitchedBuffer {
            buffer_bytes: 4096,
            ..valid_pitched_buffer()
        }));
    }

    #[test]
    fn pitched_address_checks_coordinate_bounds_and_final_element() {
        let b = valid_pitched_buffer();
        assert!(gpu_pitched_address_valid(b, 63, 31));
        assert!(!gpu_pitched_address_valid(b, 64, 31));
        assert!(!gpu_pitched_address_valid(b, 63, 32));
    }

    #[test]
    fn tensor3d_layout_and_index_are_finite() {
        let t = GpuTensor3D {
            width: 8,
            height: 4,
            depth: 2,
            element_bytes: 4,
            buffer_bytes: 256,
        };
        assert!(gpu_tensor3d_valid(t));
        assert!(gpu_tensor3d_index_valid(t, GpuTensorIndex3D { x: 7, y: 3, z: 1 }));
        assert!(!gpu_tensor3d_index_valid(t, GpuTensorIndex3D { x: 8, y: 3, z: 1 }));
        assert!(!gpu_tensor3d_valid(GpuTensor3D { buffer_bytes: 255, ..t }));
    }

    #[test]
    fn tile_projection_must_fit_inside_pitched_buffer() {
        let b = valid_pitched_buffer();
        assert!(gpu_tile_projection_valid(b, GpuTileProjection {
            origin_x: 16,
            origin_y: 8,
            rows: 8,
            cols: 16
        }));
        assert!(!gpu_tile_projection_valid(b, GpuTileProjection {
            origin_x: 60,
            origin_y: 8,
            rows: 8,
            cols: 8
        }));
        assert!(!gpu_tile_projection_valid(b, GpuTileProjection {
            origin_x: 16,
            origin_y: 30,
            rows: 4,
            cols: 8
        }));
    }

    #[test]
    fn elementwise_program_requires_capacity_for_all_elements() {
        assert!(gpu_elementwise_program_valid(GpuElementwiseProgram {
            elements: 1024,
            threads: 256,
            elements_per_thread: 4
        }));
        assert!(!gpu_elementwise_program_valid(GpuElementwiseProgram {
            elements: 1025,
            threads: 256,
            elements_per_thread: 4
        }));
        assert!(!gpu_elementwise_program_valid(GpuElementwiseProgram {
            elements: 1024,
            threads: 0,
            elements_per_thread: 4
        }));
    }

    #[test]
    fn grid_stride_loop_last_index_must_stay_in_bounds() {
        assert!(gpu_grid_stride_loop_valid(GpuGridStrideLoop {
            len: 100,
            start: 3,
            stride: 8,
            iterations: 13
        }));
        assert!(!gpu_grid_stride_loop_valid(GpuGridStrideLoop {
            len: 100,
            start: 3,
            stride: 8,
            iterations: 14
        }));
        assert!(!gpu_grid_stride_loop_valid(GpuGridStrideLoop {
            len: 100,
            start: 100,
            stride: 8,
            iterations: 1
        }));
    }

    #[test]
    fn tiled_kernel2d_grid_must_cover_output_domain() {
        assert!(gpu_tiled_kernel2d_valid(GpuTiledKernel2D {
            width: 100,
            height: 65,
            tile_width: 16,
            tile_height: 16,
            grid_x: 7,
            grid_y: 5
        }));
        assert!(!gpu_tiled_kernel2d_valid(GpuTiledKernel2D {
            width: 100,
            height: 65,
            tile_width: 16,
            tile_height: 16,
            grid_x: 6,
            grid_y: 5
        }));
        assert!(!gpu_tiled_kernel2d_valid(GpuTiledKernel2D {
            width: 100,
            height: 65,
            tile_width: 16,
            tile_height: 0,
            grid_x: 7,
            grid_y: 5
        }));
    }

    #[test]
    fn block_reduction_program_requires_input_and_shared_capacity() {
        assert!(gpu_block_reduction_program_valid(GpuBlockReductionProgram {
            input_len: 4096,
            block_threads: 256,
            elements_per_thread: 4,
            blocks: 4,
            element_bytes: 4,
            shared_capacity_bytes: 1024
        }));
        assert!(!gpu_block_reduction_program_valid(GpuBlockReductionProgram {
            input_len: 4097,
            block_threads: 256,
            elements_per_thread: 4,
            blocks: 4,
            element_bytes: 4,
            shared_capacity_bytes: 1024
        }));
        assert!(!gpu_block_reduction_program_valid(GpuBlockReductionProgram {
            input_len: 4096,
            block_threads: 256,
            elements_per_thread: 4,
            blocks: 4,
            element_bytes: 4,
            shared_capacity_bytes: 512
        }));
    }

    fn reduction_tree_program() -> GpuReductionTreeProgram {
        GpuReductionTreeProgram {
            declared_outputs: 1,
            block_program: GpuBlockReductionProgram {
                input_len: 1024,
                block_threads: 256,
                elements_per_thread: 4,
                blocks: 1,
                element_bytes: 4,
                shared_capacity_bytes: 1024,
            },
        }
    }

    fn reduction_stages() -> Vec<GpuReductionStage> {
        vec![
            GpuReductionStage {
                input_count: 1024,
                output_count: 256,
                active_lanes: 256,
                max_lanes: 256,
            },
            GpuReductionStage {
                input_count: 256,
                output_count: 32,
                active_lanes: 32,
                max_lanes: 256,
            },
            GpuReductionStage {
                input_count: 32,
                output_count: 1,
                active_lanes: 1,
                max_lanes: 256,
            },
        ]
    }

    #[test]
    fn reduction_stage_requires_positive_non_expanding_counts_and_lanes() {
        assert!(gpu_reduction_stage_valid(GpuReductionStage {
            input_count: 64,
            output_count: 32,
            active_lanes: 32,
            max_lanes: 64,
        }));
        assert!(!gpu_reduction_stage_valid(GpuReductionStage {
            input_count: 64,
            output_count: 65,
            active_lanes: 32,
            max_lanes: 64,
        }));
        assert!(!gpu_reduction_stage_valid(GpuReductionStage {
            input_count: 64,
            output_count: 32,
            active_lanes: 65,
            max_lanes: 64,
        }));
        assert!(!gpu_reduction_stage_valid(GpuReductionStage {
            input_count: 64,
            output_count: 32,
            active_lanes: 65,
            max_lanes: 128,
        }));
    }

    #[test]
    fn reduction_tree_reaches_declared_outputs_through_chain() {
        assert!(gpu_reduction_tree_program_valid(reduction_tree_program(), &reduction_stages()));
    }

    #[test]
    fn reduction_tree_rejects_empty_expanding_or_broken_chains() {
        assert!(!gpu_reduction_tree_program_valid(reduction_tree_program(), &Vec::new()));

        let mut expanding = reduction_stages();
        expanding[1].output_count = 300;
        assert!(!gpu_reduction_tree_program_valid(reduction_tree_program(), &expanding));

        let mut broken = reduction_stages();
        broken[1].input_count = 128;
        assert!(!gpu_reduction_tree_program_valid(reduction_tree_program(), &broken));

        let mut wrong_final = reduction_tree_program();
        wrong_final.declared_outputs = 2;
        assert!(!gpu_reduction_tree_program_valid(wrong_final, &reduction_stages()));
    }

    fn stencil_window(center_x: u16, center_y: u16, radius_x: u16, radius_y: u16) -> GpuStencilWindow2D {
        GpuStencilWindow2D {
            domain: GpuStencilDomain2D { width: 8, height: 6 },
            center_x,
            center_y,
            radius_x,
            radius_y,
        }
    }

    #[test]
    fn stencil_full_window_accepts_interior_reads() {
        assert!(gpu_stencil_domain2d_valid(GpuStencilDomain2D { width: 8, height: 6 }));
        assert!(gpu_stencil_full_window_valid(stencil_window(3, 3, 1, 2)));
    }

    #[test]
    fn stencil_full_window_rejects_unguarded_edges_and_oversized_radius() {
        assert!(!gpu_stencil_full_window_valid(stencil_window(0, 3, 1, 1)));
        assert!(!gpu_stencil_full_window_valid(stencil_window(3, 5, 1, 1)));
        assert!(!gpu_stencil_full_window_valid(stencil_window(3, 3, 8, 1)));
    }

    #[test]
    fn stencil_guarded_boundary_admits_only_in_domain_reads() {
        let w = stencil_window(0, 0, 1, 1);
        assert!(gpu_stencil_guarded_window_valid(w));
        assert!(gpu_stencil_guarded_read_valid(w, GpuStencilRead2D { x: 0, y: 0 }));
        assert!(gpu_stencil_guarded_read_valid(w, GpuStencilRead2D { x: 1, y: 1 }));
        assert!(!gpu_stencil_guarded_read_valid(w, GpuStencilRead2D { x: 2, y: 0 }));
        assert!(!gpu_stencil_guarded_read_valid(w, GpuStencilRead2D { x: 0, y: 2 }));
    }

    #[test]
    fn output_write_region_requires_positive_in_buffer_span() {
        assert!(gpu_output_write_region_valid(GpuOutputWriteRegion {
            base: 16,
            span_bytes: 4,
            buffer_bytes: 64,
            logical_index: 4,
        }));
        assert!(!gpu_output_write_region_valid(GpuOutputWriteRegion {
            base: 16,
            span_bytes: 0,
            buffer_bytes: 64,
            logical_index: 4,
        }));
        assert!(!gpu_output_write_region_valid(GpuOutputWriteRegion {
            base: 62,
            span_bytes: 4,
            buffer_bytes: 64,
            logical_index: 15,
        }));
    }

    #[test]
    fn output_write_disjointness_allows_same_index_and_rejects_aliasing() {
        let a = GpuOutputWriteRegion {
            base: 0,
            span_bytes: 4,
            buffer_bytes: 64,
            logical_index: 0,
        };
        let b = GpuOutputWriteRegion {
            base: 4,
            span_bytes: 4,
            buffer_bytes: 64,
            logical_index: 1,
        };
        let same = GpuOutputWriteRegion {
            base: 0,
            span_bytes: 4,
            buffer_bytes: 64,
            logical_index: 0,
        };
        let alias = GpuOutputWriteRegion {
            base: 2,
            span_bytes: 4,
            buffer_bytes: 64,
            logical_index: 1,
        };
        assert!(gpu_write_regions_disjoint_or_same_valid(a, b));
        assert!(gpu_write_regions_disjoint_or_same_valid(a, same));
        assert!(!gpu_write_regions_disjoint_or_same_valid(a, alias));
    }

    #[test]
    fn elementwise_and_tiled_output_writes_stay_in_bounds() {
        let p = GpuElementwiseProgram {
            elements: 16,
            threads: 4,
            elements_per_thread: 4,
        };
        assert!(gpu_elementwise_output_write_valid(p, 15, 4, 64));
        assert!(!gpu_elementwise_output_write_valid(p, 16, 4, 68));
        assert!(!gpu_elementwise_output_write_valid(p, 15, 4, 62));

        let k = GpuTiledKernel2D {
            width: 8,
            height: 6,
            tile_width: 4,
            tile_height: 3,
            grid_x: 2,
            grid_y: 2,
        };
        let output = GpuPitchedBuffer {
            width: 8,
            height: 6,
            element_bytes: 4,
            pitch_bytes: 32,
            buffer_bytes: 192,
        };
        assert!(gpu_tiled_output_write_valid(k, output, 7, 5));
        assert!(!gpu_tiled_output_write_valid(k, output, 8, 5));
        assert!(!gpu_tiled_output_write_valid(
            k,
            GpuPitchedBuffer {
                buffer_bytes: 190,
                ..output
            },
            7,
            5
        ));
    }
}
