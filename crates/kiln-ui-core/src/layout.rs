//! Pure responsive tiling `SplitTree`, 3-layer spatial stack, and `WorkspaceFrame` visual IR.
//!
//! Shared verbatim by Browser WASM (`#67/#68` Rats/Meteorite), Ratatui (`kiln tui`),
//! and future GPUI desktop consumers via semantic `TokenRole` enums.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::keymap::{HintEntry, Mode, PendingChord};
use crate::safety::{BatchReceiptLedger, Verb};
use crate::uri::{BufferUri, Perspective};

/// Viewport breakpoint below which the workspace renders a scrollable single-column touch stack.
pub const PHONE_MAX_PX: u16 = 1023;
/// Viewport breakpoint at or above which the workspace auto-expands from 2-pane Master+Detail to 3–4 pane tiling.
pub const WIDE_MIN_PX: u16 = 1500;
/// Maximum simultaneous panes admitted in a single `SplitTree`.
pub const MAX_PANES: usize = 6;
/// Desktop top strip height in logical px (28px preserves clean trackpad hit targets).
pub const TOP_STRIP_PX: u16 = 28;
/// Desktop bottom modeline height in logical px.
pub const MODELINE_PX: u16 = 28;
/// Desktop pane header height in logical px (28px avoids misclicks on inline overflow tabs).
pub const PANE_HEADER_PX: u16 = 28;
/// Minimum phone touch target height in logical px.
pub const TOUCH_TARGET_PX: u16 = 44;

/// Stable identifier for a pane leaf in the `SplitTree`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PaneId(pub u16);

/// Split direction in a binary space partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Responsive layout tier derived strictly from viewport width.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewportTier {
    /// `< 1024px`: single scrollable column, `44px` touch targets, no fixed-box trap.
    Phone,
    /// `1024..=1499px`: canonical 14" laptop default — 2-pane Master+Detail + inline header tabs.
    Laptop,
    /// `>= 1500px`: wide desktop — 3–4 pane BSP tiling grid.
    Wide,
}

impl ViewportTier {
    /// Classify viewport width into its canonical responsive tier.
    #[must_use]
    pub const fn from_width_px(width_px: u16) -> Self {
        if width_px <= PHONE_MAX_PX {
            Self::Phone
        } else if width_px < WIDE_MIN_PX {
            Self::Laptop
        } else {
            Self::Wide
        }
    }

    /// Maximum side-by-side visible panes before overflow splits fold into inline pane-header tabs.
    #[must_use]
    pub const fn max_visible_panes(self) -> usize {
        match self {
            Self::Phone => 1,
            Self::Laptop => 2,
            Self::Wide => 4,
        }
    }
}

/// Semantic visual token mapped 1:1 to `identity.css`, Ratatui TrueColor, and GPUI Kit.
///
/// Invariant: `BrandIdentity` (PMS 179 red) is strictly reserved for the Kiln mark/door and
/// active busy sweep — active pane focus MUST use `FocusInkBorder` + `FocusHeaderInverted`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenRole {
    BrandIdentity,
    FocusInkBorder,
    FocusHeaderInverted,
    HairlineBorder,
    EditorDarkSurface,
    StatusRunning,
    StatusSuccess,
    StatusFailure,
    StatusWarning,
    StatusQueued,
    StatusUnknown,
    StatusCancelled,
    SelectionFill,
}

/// One pane leaf holding a primary buffer plus any overflow tabs folded on narrower screens.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaneNode {
    pub id: PaneId,
    pub tabs: Vec<BufferUri>,
    pub active_tab: usize,
}

impl PaneNode {
    /// Create a single-tab pane leaf.
    #[must_use]
    pub fn single(id: PaneId, buffer: BufferUri) -> Self {
        Self {
            id,
            tabs: vec![buffer],
            active_tab: 0,
        }
    }

    /// Return the currently active buffer in this pane.
    #[must_use]
    pub fn active_buffer(&self) -> Option<&BufferUri> {
        self.tabs.get(self.active_tab).or_else(|| self.tabs.first())
    }
}

/// Binary Space Partition tiling tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum SplitTree {
    Leaf(PaneNode),
    Split {
        axis: Axis,
        /// Percentage allocated to `first` (clamped `20..=80`).
        ratio_pct: u8,
        first: Box<SplitTree>,
        second: Box<SplitTree>,
    },
}

impl SplitTree {
    /// Collect all pane IDs in deterministic in-order traversal.
    #[must_use]
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect_ids(&mut out);
        out
    }

    fn collect_ids(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf(pane) => out.push(pane.id),
            Self::Split { first, second, .. } => {
                first.collect_ids(out);
                second.collect_ids(out);
            }
        }
    }

    /// Count leaf panes in this tree.
    #[must_use]
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// Find a leaf pane immutably by ID.
    #[must_use]
    pub fn find(&self, target: PaneId) -> Option<&PaneNode> {
        match self {
            Self::Leaf(pane) if pane.id == target => Some(pane),
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => first.find(target).or_else(|| second.find(target)),
        }
    }

    /// Find a leaf pane mutably by ID.
    pub fn find_mut(&mut self, target: PaneId) -> Option<&mut PaneNode> {
        match self {
            Self::Leaf(pane) if pane.id == target => Some(pane),
            Self::Leaf(_) => None,
            Self::Split { first, second, .. } => {
                first.find_mut(target).or_else(|| second.find_mut(target))
            }
        }
    }

    /// Split `target` pane along `axis`, returning `true` if inserted (refuses past `MAX_PANES`).
    pub fn split_pane(
        &mut self,
        target: PaneId,
        axis: Axis,
        new_id: PaneId,
        new_buffer: BufferUri,
    ) -> bool {
        if self.pane_count() >= MAX_PANES {
            return false;
        }
        self.split_pane_inner(target, axis, new_id, &new_buffer)
    }

    fn split_pane_inner(
        &mut self,
        target: PaneId,
        axis: Axis,
        new_id: PaneId,
        new_buffer: &BufferUri,
    ) -> bool {
        match self {
            Self::Leaf(pane) if pane.id == target => {
                let existing = pane.clone();
                *self = Self::Split {
                    axis,
                    ratio_pct: 50,
                    first: Box::new(Self::Leaf(existing)),
                    second: Box::new(Self::Leaf(PaneNode::single(new_id, new_buffer.clone()))),
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split_pane_inner(target, axis, new_id, new_buffer)
                    || second.split_pane_inner(target, axis, new_id, new_buffer)
            }
        }
    }

    /// Close `target` pane if more than 1 pane exists, returning `true` if removed.
    pub fn close_pane(&mut self, target: PaneId) -> bool {
        if self.pane_count() <= 1 {
            return false;
        }
        let replacement = match self {
            Self::Leaf(_) => return false,
            Self::Split { first, second, .. } => {
                if matches!(first.as_ref(), Self::Leaf(p) if p.id == target) {
                    Some((**second).clone())
                } else if matches!(second.as_ref(), Self::Leaf(p) if p.id == target) {
                    Some((**first).clone())
                } else {
                    return first.close_pane(target) || second.close_pane(target);
                }
            }
        };
        if let Some(next) = replacement {
            *self = next;
            return true;
        }
        false
    }

    /// Reset all split ratios to `50%` (`Space w =`).
    pub fn equalize(&mut self) {
        if let Self::Split {
            ratio_pct,
            first,
            second,
            ..
        } = self
        {
            *ratio_pct = 50;
            first.equalize();
            second.equalize();
        }
    }

    /// Build the canonical default layout for a given perspective and responsive viewport tier.
    #[must_use]
    pub fn for_perspective(perspective: Perspective, tier: ViewportTier) -> Self {
        match (perspective, tier) {
            (_, ViewportTier::Phone) => {
                Self::Leaf(PaneNode::single(PaneId(1), perspective.default_buffer()))
            }
            (Perspective::Cockpit, ViewportTier::Laptop) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 36,
                first: Box::new(Self::Leaf(PaneNode::single(PaneId(1), BufferUri::Triage))),
                second: Box::new(Self::Leaf(PaneNode {
                    id: PaneId(2),
                    tabs: vec![
                        BufferUri::CairnIndex {
                            repo: String::new(),
                        },
                        BufferUri::Fleet,
                        BufferUri::Claims,
                    ],
                    active_tab: 0,
                })),
            },
            (Perspective::Cockpit, ViewportTier::Wide) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 45,
                first: Box::new(Self::Split {
                    axis: Axis::Horizontal,
                    ratio_pct: 55,
                    first: Box::new(Self::Leaf(PaneNode::single(PaneId(1), BufferUri::Triage))),
                    second: Box::new(Self::Leaf(PaneNode::single(
                        PaneId(3),
                        BufferUri::CairnIndex {
                            repo: String::new(),
                        },
                    ))),
                }),
                second: Box::new(Self::Split {
                    axis: Axis::Horizontal,
                    ratio_pct: 55,
                    first: Box::new(Self::Leaf(PaneNode::single(PaneId(2), BufferUri::Attempts))),
                    second: Box::new(Self::Leaf(PaneNode::single(PaneId(4), BufferUri::Fleet))),
                }),
            },
            (Perspective::Mosaic, ViewportTier::Laptop) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 50,
                first: Box::new(Self::Leaf(PaneNode::single(PaneId(1), BufferUri::Attempts))),
                second: Box::new(Self::Leaf(PaneNode {
                    id: PaneId(2),
                    tabs: vec![
                        BufferUri::SwarmBoard {
                            board: String::from("bots.general"),
                        },
                        BufferUri::Claims,
                    ],
                    active_tab: 0,
                })),
            },
            (Perspective::Mosaic, ViewportTier::Wide) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 50,
                first: Box::new(Self::Split {
                    axis: Axis::Horizontal,
                    ratio_pct: 50,
                    first: Box::new(Self::Leaf(PaneNode::single(PaneId(1), BufferUri::Attempts))),
                    second: Box::new(Self::Leaf(PaneNode::single(PaneId(3), BufferUri::Claims))),
                }),
                second: Box::new(Self::Split {
                    axis: Axis::Horizontal,
                    ratio_pct: 50,
                    first: Box::new(Self::Leaf(PaneNode::single(PaneId(2), BufferUri::Fleet))),
                    second: Box::new(Self::Leaf(PaneNode::single(
                        PaneId(4),
                        BufferUri::SwarmBoard {
                            board: String::from("bots.general"),
                        },
                    ))),
                }),
            },
            (Perspective::Review, ViewportTier::Laptop | ViewportTier::Wide) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 32,
                first: Box::new(Self::Leaf(PaneNode::single(
                    PaneId(1),
                    BufferUri::Queue {
                        repo: String::new(),
                    },
                ))),
                second: Box::new(Self::Leaf(PaneNode::single(PaneId(2), BufferUri::Runs))),
            },
            (Perspective::CiFleet, ViewportTier::Laptop) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 40,
                first: Box::new(Self::Leaf(PaneNode::single(PaneId(1), BufferUri::Runs))),
                second: Box::new(Self::Leaf(PaneNode {
                    id: PaneId(2),
                    tabs: vec![BufferUri::Fleet, BufferUri::Claims, BufferUri::Repos],
                    active_tab: 0,
                })),
            },
            (Perspective::CiFleet, ViewportTier::Wide) => Self::Split {
                axis: Axis::Vertical,
                ratio_pct: 42,
                first: Box::new(Self::Split {
                    axis: Axis::Horizontal,
                    ratio_pct: 50,
                    first: Box::new(Self::Leaf(PaneNode::single(PaneId(1), BufferUri::Runs))),
                    second: Box::new(Self::Leaf(PaneNode::single(PaneId(3), BufferUri::Fleet))),
                }),
                second: Box::new(Self::Split {
                    axis: Axis::Horizontal,
                    ratio_pct: 65,
                    first: Box::new(Self::Leaf(PaneNode::single(PaneId(2), BufferUri::Claims))),
                    second: Box::new(Self::Leaf(PaneNode::single(PaneId(4), BufferUri::Repos))),
                }),
            },
        }
    }
}

/// Steering dock target badge mode (single-target only; never blind broadcast).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SteeringMode {
    PromptAgent {
        machine: String,
        target_id: String,
    },
    AnswerDecision {
        decision_id: String,
        target_id: String,
    },
    CommentReview {
        change_id: String,
    },
}

/// Layer-1 contextual bottom dock (mutually exclusive — at most 1 active above the Modeline).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dock", rename_all = "snake_case")]
pub enum Layer1Dock {
    None,
    SteeringBar {
        target: SteeringMode,
        draft: String,
    },
    Tier2bConfirm {
        verb: Verb,
        summary: String,
        target_ids: Vec<String>,
    },
    BatchReceipts {
        ledger: BatchReceiptLedger,
    },
    CommandBar {
        input: String,
    },
    FilterBar {
        query: String,
    },
}

/// Layer-2 transient floating overlay (renders with `6px 6px 0` hard offset shadow).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "overlay", rename_all = "snake_case")]
pub enum Layer2Overlay {
    None,
    WhichKey {
        chord: PendingChord,
    },
    FuzzyPicker {
        query: String,
    },
    ActionMenu,
    Help,
    Tier3DestructiveGuard {
        verb: Verb,
        target_id: String,
        fence: Option<u64>,
        cli_command: String,
        reason: String,
        stale_shifted: bool,
    },
}

/// Deterministic render frame projected by pure `kiln-ui-core` for Browser WASM or Ratatui TUI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceFrame {
    pub perspective: Perspective,
    pub viewport_tier: ViewportTier,
    pub mode: Mode,
    pub focused_pane: PaneId,
    pub zoomed_pane: Option<PaneId>,
    pub active_focus_border: TokenRole,
    pub active_focus_header: TokenRole,
    pub top_strip_px: u16,
    pub modeline_px: u16,
    pub pane_header_px: u16,
    pub modeline_hints: Vec<HintEntry>,
    pub which_key: Vec<HintEntry>,
    pub dock: Layer1Dock,
    pub overlay: Layer2Overlay,
}
