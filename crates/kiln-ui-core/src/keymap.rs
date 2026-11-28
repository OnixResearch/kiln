//! 3-mode Helix/Emacs interaction grammar, browser-safe canonical keymap,
//! live `which-key` menu entries, and Day-1 modeline action hints.

use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::safety::Verb;
use crate::uri::{ObjectKind, Perspective};

/// Strictly 3 modes (`Esc` unconditionally returns to `Normal` everywhere).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Navigation, selection, local verbs.
    #[default]
    Normal,
    /// Pinned Layer-1 Steering / Answer / Comment / Filter bar.
    Input,
    /// `:ex` bar, `/` filter, `Space f` fuzzy picker, `Space` which-key, Tier-2b confirm.
    Command,
}

/// Multi-key chord prefix awaiting its second keystroke (always displays live `which-key`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingChord {
    Space,
    SpaceWindow,
    SpacePerspective,
    Goto,
    Prev,
    Next,
    Yank,
}

/// Platform-neutral keyboard input (mapped from DOM `KeyboardEvent` or Ratatui `KeyEvent`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    Backspace,
}

/// Keyboard modifiers.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Normalized key event passed into `WorkspaceState::reduce`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: Modifiers,
}

impl KeyEvent {
    /// Convenience constructor for a plain character key with no modifiers.
    #[must_use]
    pub const fn char(ch: char) -> Self {
        Self {
            key: Key::Char(ch),
            mods: Modifiers {
                ctrl: false,
                alt: false,
                shift: false,
                meta: false,
            },
        }
    }

    /// Convenience constructor for a non-character key.
    #[must_use]
    pub const fn bare(key: Key) -> Self {
        Self {
            key,
            mods: Modifiers {
                ctrl: false,
                alt: false,
                shift: false,
                meta: false,
            },
        }
    }
}

/// Exact graph cross-jump target (`g <key>`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphTarget {
    Cairn,
    ClaimLease,
    Attempt,
    HerdrPane,
    CiRun,
    JjDiff,
    MergeQueue,
    Machine,
    Swarm,
    CausalPrev,
    CausalNext,
}

/// Semantic prev/next target (`[` / `]`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticJump {
    Escalation,
    FailureOrHunk,
    ExpiringClaim,
    OpenDecision,
    SwarmSeq,
}

/// Window / split action (`Space w <key>`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    SplitVertical,
    SplitHorizontal,
    FocusLeft,
    FocusDown,
    FocusUp,
    FocusRight,
    FocusIndex(u8),
    FocusNext,
    FocusPrev,
    ToggleZoom,
    ClosePane,
    Equalize,
}

/// Decoded high-level command produced by the canonical keymap.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCommand {
    ReturnNormal,
    StartChord(PendingChord),
    SwitchPerspective(Perspective),
    MoveDown,
    MoveUp,
    MoveLeft,
    MoveRight,
    InspectAdjacent,
    InspectNewSplit,
    JumpBack,
    JumpForward,
    Window(WindowAction),
    SemanticPrev(SemanticJump),
    SemanticNext(SemanticJump),
    GraphJump(GraphTarget),
    SelectToggle,
    SelectRange,
    SelectAllFiltered,
    SelectCollapse,
    OpenFuzzyPicker,
    OpenBufferList,
    OpenActionMenu,
    OpenHelp,
    OpenExBar,
    OpenFilterBar,
    InvokeVerb(Verb),
    ConfirmYes,
    NoOp,
}

/// One row displayed in the live `Space` which-key overlay or Modeline top-3 hint bar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HintEntry {
    pub key: &'static str,
    pub label: &'static str,
}

/// Decode a `KeyEvent` in `Normal` mode given an optional `PendingChord`.
#[must_use]
pub fn decode_normal_key(event: KeyEvent, pending: Option<PendingChord>) -> KeyCommand {
    // Esc unconditionally cancels any chord and returns to Normal.
    if event.key == Key::Esc {
        return KeyCommand::ReturnNormal;
    }

    // Ctrl+K universal fuzzy picker, Ctrl+O / Ctrl+I jump list.
    if event.mods.ctrl && !event.mods.alt && !event.mods.meta {
        return match event.key {
            Key::Char('k') | Key::Char('K') => KeyCommand::OpenFuzzyPicker,
            Key::Char('o') => KeyCommand::JumpBack,
            Key::Char('i') => KeyCommand::JumpForward,
            _ => KeyCommand::NoOp,
        };
    }

    // Resolve active chord prefix first.
    if let Some(chord) = pending {
        return decode_chord(chord, event.key);
    }

    // Unprefixed Normal-mode keys (browser-safe: never Ctrl+1..4 or Alt+1..9).
    match event.key {
        Key::Char('1') => KeyCommand::SwitchPerspective(Perspective::Cockpit),
        Key::Char('2') => KeyCommand::SwitchPerspective(Perspective::Mosaic),
        Key::Char('3') => KeyCommand::SwitchPerspective(Perspective::Review),
        Key::Char('4') => KeyCommand::SwitchPerspective(Perspective::CiFleet),
        Key::Char('j') | Key::Down => KeyCommand::MoveDown,
        Key::Char('k') | Key::Up => KeyCommand::MoveUp,
        Key::Char('h') | Key::Left => KeyCommand::MoveLeft,
        Key::Char('l') | Key::Right => KeyCommand::MoveRight,
        Key::Enter if event.mods.shift => KeyCommand::InspectNewSplit,
        Key::Enter => KeyCommand::InspectAdjacent,
        Key::Tab => KeyCommand::Window(WindowAction::FocusNext),
        Key::BackTab => KeyCommand::Window(WindowAction::FocusPrev),
        Key::Char('z') => KeyCommand::Window(WindowAction::ToggleZoom),
        Key::Char(' ') => KeyCommand::StartChord(PendingChord::Space),
        Key::Char('g') => KeyCommand::StartChord(PendingChord::Goto),
        Key::Char('[') => KeyCommand::StartChord(PendingChord::Prev),
        Key::Char(']') => KeyCommand::StartChord(PendingChord::Next),
        Key::Char('y') => KeyCommand::StartChord(PendingChord::Yank),
        Key::Char(':') => KeyCommand::OpenExBar,
        Key::Char('/') => KeyCommand::OpenFilterBar,
        Key::Char('?') => KeyCommand::OpenHelp,
        // Helix selection-only keys (never mutate!)
        Key::Char('x') => KeyCommand::SelectToggle,
        Key::Char('X') => KeyCommand::SelectRange,
        Key::Char('%') => KeyCommand::SelectAllFiltered,
        Key::Char(',') => KeyCommand::SelectCollapse,
        // Unified contextual verbs
        Key::Char('o') => KeyCommand::InvokeVerb(Verb::OpenSplits),
        Key::Char('i') => KeyCommand::InvokeVerb(Verb::PromptAgent),
        Key::Char('n') => KeyCommand::InvokeVerb(Verb::NudgeAgentPrefill),
        Key::Char('>') => KeyCommand::InvokeVerb(Verb::PipeContext),
        Key::Char('r') => KeyCommand::InvokeVerb(Verb::RenewClaim30m),
        Key::Char('R') => KeyCommand::InvokeVerb(Verb::RenewClaim60m),
        Key::Char('p') => KeyCommand::InvokeVerb(Verb::ProbeReconcile),
        Key::Char('w') => KeyCommand::InvokeVerb(Verb::WakeCampaign),
        Key::Char('a') => KeyCommand::InvokeVerb(Verb::AcceptPlan),
        Key::Char('A') => KeyCommand::InvokeVerb(Verb::ApproveUntrustedRun),
        Key::Char('c') => KeyCommand::InvokeVerb(Verb::CommentReview),
        Key::Char('v') => KeyCommand::InvokeVerb(Verb::ReviewVerdict),
        Key::Char('m') => KeyCommand::InvokeVerb(Verb::MarkFileViewed),
        Key::Char('q') => KeyCommand::InvokeVerb(Verb::EnqueueMergeQueue),
        Key::Char('C') => KeyCommand::InvokeVerb(Verb::InterruptAttempt),
        Key::Char('D') => KeyCommand::InvokeVerb(Verb::CancelAttempt),
        _ => KeyCommand::NoOp,
    }
}

fn decode_chord(chord: PendingChord, key: Key) -> KeyCommand {
    match (chord, key) {
        // Space leader
        (PendingChord::Space, Key::Char(' ')) | (PendingChord::Space, Key::Char('f')) => {
            KeyCommand::OpenFuzzyPicker
        }
        (PendingChord::Space, Key::Char('b')) => KeyCommand::OpenBufferList,
        (PendingChord::Space, Key::Char('w')) => KeyCommand::StartChord(PendingChord::SpaceWindow),
        (PendingChord::Space, Key::Char('p')) => {
            KeyCommand::StartChord(PendingChord::SpacePerspective)
        }
        (PendingChord::Space, Key::Char('a')) => KeyCommand::OpenActionMenu,
        (PendingChord::Space, Key::Char('?')) => KeyCommand::OpenHelp,
        // Space w — window splits & focus
        (PendingChord::SpaceWindow, Key::Char('v')) => {
            KeyCommand::Window(WindowAction::SplitVertical)
        }
        (PendingChord::SpaceWindow, Key::Char('s')) => {
            KeyCommand::Window(WindowAction::SplitHorizontal)
        }
        (PendingChord::SpaceWindow, Key::Char('h')) => KeyCommand::Window(WindowAction::FocusLeft),
        (PendingChord::SpaceWindow, Key::Char('j')) => KeyCommand::Window(WindowAction::FocusDown),
        (PendingChord::SpaceWindow, Key::Char('k')) => KeyCommand::Window(WindowAction::FocusUp),
        (PendingChord::SpaceWindow, Key::Char('l')) => KeyCommand::Window(WindowAction::FocusRight),
        (PendingChord::SpaceWindow, Key::Char('1')) => {
            KeyCommand::Window(WindowAction::FocusIndex(1))
        }
        (PendingChord::SpaceWindow, Key::Char('2')) => {
            KeyCommand::Window(WindowAction::FocusIndex(2))
        }
        (PendingChord::SpaceWindow, Key::Char('3')) => {
            KeyCommand::Window(WindowAction::FocusIndex(3))
        }
        (PendingChord::SpaceWindow, Key::Char('4')) => {
            KeyCommand::Window(WindowAction::FocusIndex(4))
        }
        (PendingChord::SpaceWindow, Key::Char('z')) => KeyCommand::Window(WindowAction::ToggleZoom),
        (PendingChord::SpaceWindow, Key::Char('q')) => KeyCommand::Window(WindowAction::ClosePane),
        (PendingChord::SpaceWindow, Key::Char('=')) => KeyCommand::Window(WindowAction::Equalize),
        // Space p — perspectives
        (PendingChord::SpacePerspective, Key::Char('1')) => {
            KeyCommand::SwitchPerspective(Perspective::Cockpit)
        }
        (PendingChord::SpacePerspective, Key::Char('2')) => {
            KeyCommand::SwitchPerspective(Perspective::Mosaic)
        }
        (PendingChord::SpacePerspective, Key::Char('3')) => {
            KeyCommand::SwitchPerspective(Perspective::Review)
        }
        (PendingChord::SpacePerspective, Key::Char('4')) => {
            KeyCommand::SwitchPerspective(Perspective::CiFleet)
        }
        // g — exact graph cross-jumps (locked: g c = Cairn, g l = Lease/Claim, g s = Swarm)
        (PendingChord::Goto, Key::Char('c')) => KeyCommand::GraphJump(GraphTarget::Cairn),
        (PendingChord::Goto, Key::Char('l')) => KeyCommand::GraphJump(GraphTarget::ClaimLease),
        (PendingChord::Goto, Key::Char('a')) => KeyCommand::GraphJump(GraphTarget::Attempt),
        (PendingChord::Goto, Key::Char('p')) => KeyCommand::GraphJump(GraphTarget::HerdrPane),
        (PendingChord::Goto, Key::Char('r')) => KeyCommand::GraphJump(GraphTarget::CiRun),
        (PendingChord::Goto, Key::Char('d')) => KeyCommand::GraphJump(GraphTarget::JjDiff),
        (PendingChord::Goto, Key::Char('q')) => KeyCommand::GraphJump(GraphTarget::MergeQueue),
        (PendingChord::Goto, Key::Char('m')) => KeyCommand::GraphJump(GraphTarget::Machine),
        (PendingChord::Goto, Key::Char('s')) => KeyCommand::GraphJump(GraphTarget::Swarm),
        (PendingChord::Goto, Key::Char('[')) => KeyCommand::GraphJump(GraphTarget::CausalPrev),
        (PendingChord::Goto, Key::Char(']')) => KeyCommand::GraphJump(GraphTarget::CausalNext),
        // [ / ] — semantic prev / next
        (PendingChord::Prev, Key::Char('e')) => KeyCommand::SemanticPrev(SemanticJump::Escalation),
        (PendingChord::Prev, Key::Char('f')) => {
            KeyCommand::SemanticPrev(SemanticJump::FailureOrHunk)
        }
        (PendingChord::Prev, Key::Char('c')) => {
            KeyCommand::SemanticPrev(SemanticJump::ExpiringClaim)
        }
        (PendingChord::Prev, Key::Char('d')) => {
            KeyCommand::SemanticPrev(SemanticJump::OpenDecision)
        }
        (PendingChord::Prev, Key::Char('s')) => KeyCommand::SemanticPrev(SemanticJump::SwarmSeq),
        (PendingChord::Next, Key::Char('e')) => KeyCommand::SemanticNext(SemanticJump::Escalation),
        (PendingChord::Next, Key::Char('f')) => {
            KeyCommand::SemanticNext(SemanticJump::FailureOrHunk)
        }
        (PendingChord::Next, Key::Char('c')) => {
            KeyCommand::SemanticNext(SemanticJump::ExpiringClaim)
        }
        (PendingChord::Next, Key::Char('d')) => {
            KeyCommand::SemanticNext(SemanticJump::OpenDecision)
        }
        (PendingChord::Next, Key::Char('s')) => KeyCommand::SemanticNext(SemanticJump::SwarmSeq),
        // y — yank exact identity / URL / attach / CLI
        (PendingChord::Yank, Key::Char('i')) => KeyCommand::InvokeVerb(Verb::YankIdentity),
        (PendingChord::Yank, Key::Char('u')) => KeyCommand::InvokeVerb(Verb::YankUrl),
        (PendingChord::Yank, Key::Char('a')) => KeyCommand::InvokeVerb(Verb::YankAttach),
        (PendingChord::Yank, Key::Char('c')) => KeyCommand::InvokeVerb(Verb::YankCli),
        _ => KeyCommand::ReturnNormal,
    }
}

/// Live `which-key` popup entries for any pending chord.
#[must_use]
pub fn which_key_entries(chord: PendingChord) -> Vec<HintEntry> {
    match chord {
        PendingChord::Space => vec![
            HintEntry {
                key: "f",
                label: "fuzzy picker",
            },
            HintEntry {
                key: "a",
                label: "all actions",
            },
            HintEntry {
                key: "w",
                label: "splits & zoom",
            },
            HintEntry {
                key: "p",
                label: "perspectives",
            },
            HintEntry {
                key: "b",
                label: "open buffers",
            },
            HintEntry {
                key: "?",
                label: "pane help",
            },
        ],
        PendingChord::SpaceWindow => vec![
            HintEntry {
                key: "v",
                label: "vertical split",
            },
            HintEntry {
                key: "s",
                label: "horizontal split",
            },
            HintEntry {
                key: "h/j/k/l",
                label: "focus pane",
            },
            HintEntry {
                key: "z",
                label: "zoom toggle",
            },
            HintEntry {
                key: "q",
                label: "close pane",
            },
            HintEntry {
                key: "=",
                label: "equalize",
            },
        ],
        PendingChord::SpacePerspective => vec![
            HintEntry {
                key: "1",
                label: "Cockpit",
            },
            HintEntry {
                key: "2",
                label: "Agent Mosaic",
            },
            HintEntry {
                key: "3",
                label: "Review & jj",
            },
            HintEntry {
                key: "4",
                label: "CI & Fleet",
            },
        ],
        PendingChord::Goto => vec![
            HintEntry {
                key: "c",
                label: "Cairn spec",
            },
            HintEntry {
                key: "l",
                label: "claim/lease",
            },
            HintEntry {
                key: "a",
                label: "attempt",
            },
            HintEntry {
                key: "p",
                label: "Herdr pane",
            },
            HintEntry {
                key: "r",
                label: "CI run",
            },
            HintEntry {
                key: "d",
                label: "jj diff",
            },
            HintEntry {
                key: "q",
                label: "merge queue",
            },
            HintEntry {
                key: "m",
                label: "machine",
            },
            HintEntry {
                key: "s",
                label: "swarm board",
            },
            HintEntry {
                key: "[ / ]",
                label: "causal spine",
            },
        ],
        PendingChord::Prev | PendingChord::Next => vec![
            HintEntry {
                key: "e",
                label: "escalation",
            },
            HintEntry {
                key: "f",
                label: "failure / hunk",
            },
            HintEntry {
                key: "c",
                label: "expiring claim",
            },
            HintEntry {
                key: "d",
                label: "open decision",
            },
            HintEntry {
                key: "s",
                label: "swarm seq",
            },
        ],
        PendingChord::Yank => vec![
            HintEntry {
                key: "i",
                label: "BLAKE3 / fence",
            },
            HintEntry {
                key: "u",
                label: "canonical URL",
            },
            HintEntry {
                key: "a",
                label: "herdr attach",
            },
            HintEntry {
                key: "c",
                label: "CLI command",
            },
        ],
    }
}

/// Top 3 contextual single-key hints echoed permanently in the 28px Modeline for Day-1 zero memorization.
#[must_use]
pub fn modeline_hints(kind: Option<ObjectKind>, multi_select: bool) -> Vec<HintEntry> {
    if multi_select {
        return vec![
            HintEntry {
                key: "r",
                label: "renew +30m",
            },
            HintEntry {
                key: "p",
                label: "probe",
            },
            HintEntry {
                key: "o",
                label: "open splits",
            },
            HintEntry {
                key: ",",
                label: "clear sel",
            },
        ];
    }
    match kind {
        Some(ObjectKind::Decision) => vec![
            HintEntry {
                key: "i",
                label: "answer",
            },
            HintEntry {
                key: "g p",
                label: "Herdr pane",
            },
            HintEntry {
                key: "Space a",
                label: "all actions",
            },
        ],
        Some(ObjectKind::Claim) => vec![
            HintEntry {
                key: "r",
                label: "+30m",
            },
            HintEntry {
                key: "R",
                label: "+60m",
            },
            HintEntry {
                key: "D",
                label: "release",
            },
        ],
        Some(ObjectKind::Attempt) | Some(ObjectKind::HerdrPane) => vec![
            HintEntry {
                key: "i",
                label: "prompt",
            },
            HintEntry {
                key: "n",
                label: "nudge",
            },
            HintEntry {
                key: "C",
                label: "interrupt",
            },
        ],
        Some(ObjectKind::CiRun) => vec![
            HintEntry {
                key: "r",
                label: "rerun",
            },
            HintEntry {
                key: ">",
                label: "pipe to agent",
            },
            HintEntry {
                key: "p",
                label: "reconcile",
            },
        ],
        Some(ObjectKind::JjChange) => vec![
            HintEntry {
                key: "c",
                label: "comment",
            },
            HintEntry {
                key: "v",
                label: "verdict",
            },
            HintEntry {
                key: "q",
                label: "enqueue MQ",
            },
        ],
        Some(ObjectKind::Machine) => vec![
            HintEntry {
                key: "g p",
                label: "Herdr panes",
            },
            HintEntry {
                key: "Space a",
                label: "drain / ops",
            },
            HintEntry {
                key: "z",
                label: "zoom",
            },
        ],
        _ => vec![
            HintEntry {
                key: "Space f",
                label: "picker",
            },
            HintEntry {
                key: "Space a",
                label: "actions",
            },
            HintEntry {
                key: "?",
                label: "keys",
            },
        ],
    }
}
