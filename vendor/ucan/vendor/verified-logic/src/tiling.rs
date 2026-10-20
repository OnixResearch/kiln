//! Verified tiling layout primitives.
//!
//! Models 2D rectangle placement for tiling window managers: non-overlap,
//! containment, positive area, and 1D strip partitioning. All coordinates
//! are u16 cell positions (matching terminal/TUI coordinate spaces).
//!
//! Builds on `interval` and `region` for 1D reasoning, lifting to 2D rects.

#![allow(clippy::nonminimal_bool)]

use vstd::prelude::*;

verus! {

// --- Rect as (x, y, width, height) ---

// r[depends tiling.rect-end]
/// Spec: exclusive right edge of a rect.
pub open spec fn right_spec(x: u16, width: u16) -> int {
    x as int + width as int
}

/// Spec: exclusive bottom edge of a rect.
pub open spec fn bottom_spec(y: u16, height: u16) -> int {
    y as int + height as int
}

// r[impl tiling.rect-end]
/// Returns the exclusive right edge x + width.
pub fn right(x: u16, width: u16) -> (result: u32)
    ensures result as int == right_spec(x, width)
{
    x as u32 + width as u32
}

/// Returns the exclusive bottom edge y + height.
pub fn bottom(y: u16, height: u16) -> (result: u32)
    ensures result as int == bottom_spec(y, height)
{
    y as u32 + height as u32
}

// --- has_area ---

// r[depends tiling.has-area]
/// Spec: rect has positive area.
pub open spec fn has_area_spec(width: u16, height: u16) -> bool {
    width > 0 && height > 0
}

// r[impl tiling.has-area]
/// Returns true if the rect has positive area (width > 0 and height > 0).
pub fn has_area(width: u16, height: u16) -> (result: bool)
    ensures result == has_area_spec(width, height)
{
    width > 0 && height > 0
}

// r[verify tiling.has-area]
/// Zero width or zero height means no area.
proof fn zero_dim_no_area(width: u16, height: u16)
    requires width == 0 || height == 0
    ensures !has_area_spec(width, height)
{}

// --- rects_disjoint ---

// r[depends tiling.disjoint]
/// Spec: two rects do not overlap. True when either rect has no area
/// (contains no points) or they are separated on at least one axis.
/// Uses int arithmetic to avoid overflow.
pub open spec fn rects_disjoint_spec(
    ax: u16, ay: u16, aw: u16, ah: u16,
    bx: u16, by: u16, bw: u16, bh: u16,
) -> bool {
    !has_area_spec(aw, ah)
    || !has_area_spec(bw, bh)
    || right_spec(ax, aw) <= bx as int
    || right_spec(bx, bw) <= ax as int
    || bottom_spec(ay, ah) <= by as int
    || bottom_spec(by, bh) <= ay as int
}

// r[impl tiling.disjoint]
/// Returns true if the two rects do not overlap.
pub fn rects_disjoint(
    ax: u16, ay: u16, aw: u16, ah: u16,
    bx: u16, by: u16, bw: u16, bh: u16,
) -> (result: bool)
    ensures result == rects_disjoint_spec(ax, ay, aw, ah, bx, by, bw, bh)
{
    !has_area(aw, ah) || !has_area(bw, bh) || {
        let ar = right(ax, aw);
        let ab = bottom(ay, ah);
        let br = right(bx, bw);
        let bb = bottom(by, bh);
        ar <= bx as u32 || br <= ax as u32 || ab <= by as u32 || bb <= ay as u32
    }
}

// r[verify tiling.disjoint]
/// Disjointness is symmetric.
proof fn disjoint_symmetric(
    ax: u16, ay: u16, aw: u16, ah: u16,
    bx: u16, by: u16, bw: u16, bh: u16,
)
    ensures rects_disjoint_spec(ax, ay, aw, ah, bx, by, bw, bh)
        == rects_disjoint_spec(bx, by, bw, bh, ax, ay, aw, ah)
{}

// r[verify tiling.disjoint]
/// A zero-area rect is disjoint from everything.
proof fn empty_disjoint(
    ax: u16, ay: u16, aw: u16, ah: u16,
    bx: u16, by: u16, bw: u16, bh: u16,
)
    requires !has_area_spec(aw, ah)
    ensures rects_disjoint_spec(ax, ay, aw, ah, bx, by, bw, bh)
{}

// r[verify tiling.disjoint]
/// A rect with positive area is not disjoint from itself.
proof fn positive_area_not_self_disjoint(x: u16, y: u16, w: u16, h: u16)
    requires has_area_spec(w, h)
    ensures !rects_disjoint_spec(x, y, w, h, x, y, w, h)
{}

// --- contained_in ---

// r[depends tiling.contained]
/// Spec: inner rect is fully contained within outer rect.
pub open spec fn contained_in_spec(
    ix: u16, iy: u16, iw: u16, ih: u16,
    ox: u16, oy: u16, ow: u16, oh: u16,
) -> bool {
    ix as int >= ox as int
    && iy as int >= oy as int
    && right_spec(ix, iw) <= right_spec(ox, ow)
    && bottom_spec(iy, ih) <= bottom_spec(oy, oh)
}

// r[impl tiling.contained]
/// Returns true if the inner rect fits entirely within the outer rect.
pub fn contained_in(
    ix: u16, iy: u16, iw: u16, ih: u16,
    ox: u16, oy: u16, ow: u16, oh: u16,
) -> (result: bool)
    ensures result == contained_in_spec(ix, iy, iw, ih, ox, oy, ow, oh)
{
    ix >= ox && iy >= oy
        && right(ix, iw) <= right(ox, ow)
        && bottom(iy, ih) <= bottom(oy, oh)
}

// r[verify tiling.contained]
/// Containment is reflexive.
proof fn contained_reflexive(x: u16, y: u16, w: u16, h: u16)
    ensures contained_in_spec(x, y, w, h, x, y, w, h)
{}

// r[verify tiling.contained]
/// Containment is transitive.
proof fn contained_transitive(
    ax: u16, ay: u16, aw: u16, ah: u16,
    bx: u16, by: u16, bw: u16, bh: u16,
    cx: u16, cy: u16, cw: u16, ch: u16,
)
    requires
        contained_in_spec(ax, ay, aw, ah, bx, by, bw, bh),
        contained_in_spec(bx, by, bw, bh, cx, cy, cw, ch),
    ensures contained_in_spec(ax, ay, aw, ah, cx, cy, cw, ch)
{}

// r[verify tiling.contained]
/// A zero-area rect at the origin is contained in any rect at the origin.
proof fn empty_at_origin_contained(ow: u16, oh: u16)
    ensures contained_in_spec(0, 0, 0, 0, 0, 0, ow, oh)
{}

// r[verify tiling.contained]
/// If inner is contained and has area, outer has area too.
proof fn contained_with_area_implies_outer_area(
    ix: u16, iy: u16, iw: u16, ih: u16,
    ox: u16, oy: u16, ow: u16, oh: u16,
)
    requires
        contained_in_spec(ix, iy, iw, ih, ox, oy, ow, oh),
        has_area_spec(iw, ih),
    ensures has_area_spec(ow, oh)
{}

// --- contained implies disjoint complement ---

// r[verify tiling.contained]
// r[verify tiling.disjoint]
/// If inner is contained in outer, and other is disjoint from outer,
/// then other is disjoint from inner.
proof fn disjoint_from_container_implies_disjoint(
    ix: u16, iy: u16, iw: u16, ih: u16,
    ox: u16, oy: u16, ow: u16, oh: u16,
    dx: u16, dy: u16, dw: u16, dh: u16,
)
    requires
        contained_in_spec(ix, iy, iw, ih, ox, oy, ow, oh),
        rects_disjoint_spec(ox, oy, ow, oh, dx, dy, dw, dh),
    ensures rects_disjoint_spec(ix, iy, iw, ih, dx, dy, dw, dh)
{}

// --- 1D strip partitioning ---

// r[depends tiling.strip-adjacent]
/// Spec: two 1D segments are adjacent (one ends where the next begins).
pub open spec fn segments_adjacent_spec(a_pos: u16, a_size: u16, b_pos: u16) -> bool {
    a_pos as int + a_size as int == b_pos as int
}

// r[impl tiling.strip-adjacent]
/// Returns true if segment a ends exactly where segment b begins.
pub fn segments_adjacent(a_pos: u16, a_size: u16, b_pos: u16) -> (result: bool)
    ensures result == segments_adjacent_spec(a_pos, a_size, b_pos)
{
    a_pos as u32 + a_size as u32 == b_pos as u32
}

// r[verify tiling.strip-adjacent]
/// Adjacent segments don't overlap on the 1D axis.
proof fn adjacent_no_1d_overlap(a_pos: u16, a_size: u16, b_pos: u16, b_size: u16)
    requires
        segments_adjacent_spec(a_pos, a_size, b_pos),
        a_size > 0,
        b_size > 0,
    ensures
        // They don't share any point: [a_pos, a_pos+a_size) ∩ [b_pos, b_pos+b_size) = ∅
        a_pos as int + a_size as int <= b_pos as int
            || b_pos as int + b_size as int <= a_pos as int
{}

// --- gapped strip ---

// r[depends tiling.gapped-position]
/// Spec: position of item i in a strip with uniform sizes and gaps.
/// pos(i) = i * (size + gap).
pub open spec fn gapped_position_spec(index: u16, size: u16, gap: u16) -> int {
    index as int * (size as int + gap as int)
}

// r[impl tiling.gapped-position]
// r[verify tiling.gapped-position]
/// Items at different indices in a uniform gapped strip are disjoint.
proof fn gapped_items_disjoint(i: u16, j: u16, size: u16, gap: u16)
    requires i < j, size > 0, gap >= 0
    ensures
        gapped_position_spec(i, size, gap) + size as int
            <= gapped_position_spec(j, size, gap)
{
    // pos(i) + size = i*(size+gap) + size
    // pos(j) = j*(size+gap) >= (i+1)*(size+gap) = i*(size+gap) + size + gap
    // So pos(i) + size <= pos(j) when gap >= 0.
    assert(gapped_position_spec(j, size, gap) >= gapped_position_spec(i, size, gap)
        + (size as int + gap as int)) by(nonlinear_arith)
        requires i < j, size > 0, gap >= 0,
            gapped_position_spec(i, size, gap) == i as int * (size as int + gap as int),
            gapped_position_spec(j, size, gap) == j as int * (size as int + gap as int);
}

// r[verify tiling.gapped-position]
/// Position of the first item is zero.
proof fn first_item_at_zero(size: u16, gap: u16)
    ensures gapped_position_spec(0, size, gap) == 0
{}

// --- viewport clipping ---

// r[depends tiling.clip]
/// Spec: clip a 1D segment to a viewport [vp_start, vp_start + vp_size).
/// Returns (clipped_start, clipped_size) or (0, 0) if fully outside.
pub open spec fn clip_to_viewport_spec(
    seg_pos: u16, seg_size: u16,
    vp_start: u16, vp_size: u16,
) -> (int, int) {
    let seg_end = seg_pos as int + seg_size as int;
    let vp_end = vp_start as int + vp_size as int;
    if seg_end <= vp_start as int || seg_pos as int >= vp_end {
        (0, 0) // fully outside
    } else {
        let clip_start = if seg_pos as int >= vp_start as int { seg_pos as int } else { vp_start as int };
        let clip_end = if seg_end <= vp_end { seg_end } else { vp_end };
        (clip_start, clip_end - clip_start)
    }
}

// r[impl tiling.clip]
// r[verify tiling.clip]
/// A fully visible segment clips to itself.
proof fn clip_identity_when_visible(
    seg_pos: u16, seg_size: u16, vp_start: u16, vp_size: u16,
)
    requires
        seg_size > 0,
        vp_size > 0,
        seg_pos as int >= vp_start as int,
        seg_pos as int + seg_size as int <= vp_start as int + vp_size as int,
    ensures clip_to_viewport_spec(seg_pos, seg_size, vp_start, vp_size)
        == (seg_pos as int, seg_size as int)
{
    // seg_end = seg_pos + seg_size > seg_pos >= vp_start, so not fully left
    // seg_pos < seg_pos + seg_size <= vp_start + vp_size = vp_end, so not fully right
}

// r[verify tiling.clip]
/// A fully outside segment clips to zero size.
proof fn clip_outside_is_zero(
    seg_pos: u16, seg_size: u16, vp_start: u16, vp_size: u16,
)
    requires
        seg_pos as int + seg_size as int <= vp_start as int
        || seg_pos as int >= vp_start as int + vp_size as int,
    ensures clip_to_viewport_spec(seg_pos, seg_size, vp_start, vp_size) == (0int, 0int)
{}

// r[verify tiling.clip]
/// Clipped size is non-negative.
proof fn clip_size_nonneg(
    seg_pos: u16, seg_size: u16, vp_start: u16, vp_size: u16,
)
    ensures clip_to_viewport_spec(seg_pos, seg_size, vp_start, vp_size).1 >= 0
{}

// r[verify tiling.clip]
/// Clipped segment fits within the viewport.
proof fn clip_fits_viewport(
    seg_pos: u16, seg_size: u16, vp_start: u16, vp_size: u16,
)
    ensures ({
        let (cs, cl) = clip_to_viewport_spec(seg_pos, seg_size, vp_start, vp_size);
        cl == 0 || (cs >= vp_start as int && cs + cl <= vp_start as int + vp_size as int)
    })
{}

// --- scroll bounds ---

// r[depends tiling.scroll-bounds]
/// Spec: scroll offset is valid (viewport doesn't extend past content).
pub open spec fn scroll_in_bounds_spec(
    scroll_offset: u16, viewport_size: u16, content_extent: u16,
) -> bool {
    scroll_offset as int + viewport_size as int <= content_extent as int
    || content_extent <= viewport_size
}

// r[impl tiling.scroll-bounds]
// r[verify tiling.scroll-bounds]
/// Zero offset is always valid when content fits in viewport.
proof fn zero_scroll_when_fits(viewport_size: u16, content_extent: u16)
    requires content_extent <= viewport_size
    ensures scroll_in_bounds_spec(0, viewport_size, content_extent)
{}

// r[verify tiling.scroll-bounds]
/// Maximum valid scroll offset.
proof fn max_scroll_offset(viewport_size: u16, content_extent: u16)
    requires content_extent >= viewport_size
    ensures scroll_in_bounds_spec(
        (content_extent - viewport_size) as u16,
        viewport_size,
        content_extent,
    )
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify tiling.has-area]
    // r[verify tiling.rect-end]
    #[test]
    fn test_has_area() {
        assert!(has_area(10, 20));
        assert!(!has_area(0, 20));
        assert!(!has_area(10, 0));
        assert!(!has_area(0, 0));
    }

    // r[verify tiling.disjoint]
    #[test]
    fn test_rects_disjoint() {
        // Side by side
        assert!(rects_disjoint(0, 0, 10, 10, 10, 0, 10, 10));
        // Stacked
        assert!(rects_disjoint(0, 0, 10, 10, 0, 10, 10, 10));
        // Overlapping
        assert!(!rects_disjoint(0, 0, 10, 10, 5, 5, 10, 10));
        // Identical
        assert!(!rects_disjoint(0, 0, 10, 10, 0, 0, 10, 10));
        // Zero-area (both dims zero)
        assert!(rects_disjoint(5, 5, 0, 0, 0, 0, 10, 10));
        // Zero-width (still no area — disjoint from everything)
        assert!(rects_disjoint(5, 5, 0, 10, 0, 0, 10, 10));
        // Zero-height
        assert!(rects_disjoint(5, 5, 10, 0, 0, 0, 10, 10));
    }

    // r[verify tiling.contained]
    #[test]
    fn test_contained_in() {
        assert!(contained_in(2, 2, 5, 5, 0, 0, 10, 10));
        assert!(contained_in(0, 0, 10, 10, 0, 0, 10, 10)); // equal
        assert!(!contained_in(0, 0, 10, 10, 2, 2, 5, 5)); // too big
        assert!(contained_in(0, 0, 0, 0, 0, 0, 10, 10)); // zero-area
    }

    // r[verify tiling.strip-adjacent]
    #[test]
    fn test_segments_adjacent() {
        assert!(segments_adjacent(0, 10, 10));
        assert!(segments_adjacent(5, 15, 20));
        assert!(!segments_adjacent(0, 10, 11)); // gap
        assert!(!segments_adjacent(0, 10, 9)); // overlap
    }

    proptest! {
        // r[verify tiling.disjoint]
        #[test]
        fn prop_disjoint_symmetric(
            ax in 0u16..500, ay in 0u16..500, aw in 0u16..200, ah in 0u16..200,
            bx in 0u16..500, by in 0u16..500, bw in 0u16..200, bh in 0u16..200,
        ) {
            prop_assert_eq!(
                rects_disjoint(ax, ay, aw, ah, bx, by, bw, bh),
                rects_disjoint(bx, by, bw, bh, ax, ay, aw, ah),
            );
        }

        // r[verify tiling.contained]
        #[test]
        fn prop_contained_reflexive(
            x in 0u16..500, y in 0u16..500, w in 0u16..200, h in 0u16..200,
        ) {
            prop_assert!(contained_in(x, y, w, h, x, y, w, h));
        }

        // r[verify tiling.contained]
        // r[verify tiling.disjoint]
        #[test]
        fn prop_contained_and_disjoint_from_outer(
            ix in 0u16..100, iy in 0u16..100, iw in 1u16..50, ih in 1u16..50,
            ox in 0u16..100, oy in 0u16..100, ow in 1u16..200, oh in 1u16..200,
            dx in 0u16..500, dy in 0u16..500, dw in 1u16..200, dh in 1u16..200,
        ) {
            if contained_in(ix, iy, iw, ih, ox, oy, ow, oh)
                && rects_disjoint(ox, oy, ow, oh, dx, dy, dw, dh)
            {
                prop_assert!(rects_disjoint(ix, iy, iw, ih, dx, dy, dw, dh));
            }
        }

        // r[verify tiling.has-area]
        // r[verify tiling.disjoint]
        #[test]
        fn prop_empty_always_disjoint(
            ax in 0u16..500, ay in 0u16..500,
            bx in 0u16..500, by in 0u16..500, bw in 0u16..200, bh in 0u16..200,
        ) {
            // Zero width
            prop_assert!(rects_disjoint(ax, ay, 0, 10, bx, by, bw, bh));
            // Zero height
            prop_assert!(rects_disjoint(ax, ay, 10, 0, bx, by, bw, bh));
        }

        // r[verify tiling.strip-adjacent]
        #[test]
        fn prop_adjacent_segments_no_overlap(
            a_pos in 0u16..500, a_size in 1u16..200, _b_size in 1u16..200,
        ) {
            let b_pos = a_pos.checked_add(a_size);
            if let Some(b_pos) = b_pos {
                if segments_adjacent(a_pos, a_size, b_pos) {
                    // a ends exactly where b begins
                    let a_end = a_pos as u32 + a_size as u32;
                    prop_assert_eq!(a_end, b_pos as u32);
                }
            }
        }
    }
}
