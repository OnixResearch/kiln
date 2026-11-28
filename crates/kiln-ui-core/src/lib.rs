//! Pure `no_std + alloc` core state machine for Kiln's Helix/Emacs-grade AI Software Factory UI.
//!
//! Zero I/O, zero DOM, zero terminal coupling, zero provider imports.
//! Thin imperative shells (`#67`/`#68` Rats/Meteorite WASM, Ratatui `kiln tui`, or future GPUI)
//! feed [`UiEvent`] into [`WorkspaceState::reduce`], execute returned [`UiEffect`]s, and render
//! [`WorkspaceState::frame`].

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod keymap;
pub mod layout;
pub mod safety;
pub mod selection;
pub mod uri;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::keymap::{
    GraphTarget, Key, KeyCommand, KeyEvent, Mode, PendingChord, SemanticJump, WindowAction,
    decode_normal_key, modeline_hints, which_key_entries,
};
use crate::layout::{
    Axis, Layer1Dock, Layer2Overlay, MODELINE_PX, PANE_HEADER_PX, PaneId, SplitTree, SteeringMode,
    TOP_STRIP_PX, TOUCH_TARGET_PX, TokenRole, ViewportTier, WorkspaceFrame,
};
use crate::safety::{
    BatchReceipt, BatchReceiptLedger, FreshnessGuard, MutationBlockReason, PendingMutation,
    ReceiptStatus, RunOutcome, SafetyTier, Verb, check_run_verb,
};
use crate::selection::{SelectionSet, TypedRef};
use crate::uri::{BufferUri, ObjectKind, Perspective};

/// Maximum saved entries in the exact-identity jump list (`C-o` / `C-i`).
pub const MAX_JUMP_HISTORY: usize = 64;

/// Default nudge template pre-filled into the Steering Bar (`n` key — never blind 1-key fire!).
pub const DEFAULT_NUDGE_TEMPLATE: &str =
    "Status check: summarize blockers or hand off current progress.";

/// Events fed into the pure workspace reducer by any thin shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiEvent {
    Key(KeyEvent),
    Resize {
        width_px: u16,
    },
    SetOperatorAuthority {
        can_operate: bool,
    },
    SseStatusChanged {
        connected: bool,
    },
    BufferObserved {
        uri: BufferUri,
        observed_at_ms: u64,
    },
    CursorChanged {
        target: Option<TypedRef>,
        run_outcome: Option<RunOutcome>,
    },
    MutationSettled {
        request_id: String,
        target_id: String,
        fence: Option<u64>,
        status: ReceiptStatus,
    },
    RestoreFromQuery {
        query: String,
    },
}

/// Requested side effects returned by pure [`WorkspaceState::reduce`] for the shell to execute.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum UiEffect {
    SyncUrl {
        query: String,
    },
    MoveCursor {
        delta: i8,
    },
    InspectTarget {
        target: TypedRef,
        new_split: bool,
    },
    GraphCrossJump {
        from: Option<TypedRef>,
        target: GraphTarget,
    },
    SemanticNavigate {
        direction: i8,
        target: SemanticJump,
    },
    YankToClipboard {
        verb: Verb,
        targets: Vec<TypedRef>,
    },
    DispatchMutation {
        request_id: String,
        verb: Verb,
        targets: Vec<TypedRef>,
        payload: Option<String>,
    },
    ShowBlockedNotice {
        reason: MutationBlockReason,
    },
    AdvanceCursorAfterSuccess {
        target_id: String,
    },
}

/// Complete deterministic state of one Kiln Factory workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceState {
    pub perspective: Perspective,
    pub viewport_tier: ViewportTier,
    pub mode: Mode,
    pub pending_chord: Option<PendingChord>,
    pub layout: SplitTree,
    pub focused_pane: PaneId,
    pub zoomed_pane: Option<PaneId>,
    pub next_pane_id: u16,
    pub can_operate: bool,
    pub sse_connected: bool,
    pub buffer_observed_ms: BTreeMap<String, u64>,
    pub selection: SelectionSet,
    pub cursor_run_outcome: Option<RunOutcome>,
    pub pending_mutation: Option<PendingMutation>,
    pub next_request_seq: u64,
    pub drafts: BTreeMap<String, String>,
    pub dock: Layer1Dock,
    pub overlay: Layer2Overlay,
    pub jump_history: Vec<(Perspective, BufferUri)>,
    pub jump_index: usize,
    pub last_blocked: Option<MutationBlockReason>,
    pub receipt_ledger: BatchReceiptLedger,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new(1280)
    }
}

impl WorkspaceState {
    /// Initialize a clean workspace sized for `width_px` (defaults to `1:Cockpit`).
    #[must_use]
    pub fn new(width_px: u16) -> Self {
        let viewport_tier = ViewportTier::from_width_px(width_px);
        let perspective = Perspective::Cockpit;
        let layout = SplitTree::for_perspective(perspective, viewport_tier);
        let initial_buf = perspective.default_buffer();
        Self {
            perspective,
            viewport_tier,
            mode: Mode::Normal,
            pending_chord: None,
            layout,
            focused_pane: PaneId(1),
            zoomed_pane: None,
            next_pane_id: 5,
            can_operate: false,
            sse_connected: false,
            buffer_observed_ms: BTreeMap::new(),
            selection: SelectionSet::default(),
            cursor_run_outcome: None,
            pending_mutation: None,
            next_request_seq: 1,
            drafts: BTreeMap::new(),
            dock: Layer1Dock::None,
            overlay: Layer2Overlay::None,
            jump_history: vec![(perspective, initial_buf)],
            jump_index: 0,
            last_blocked: None,
            receipt_ledger: BatchReceiptLedger::default(),
        }
    }

    /// Return the currently focused buffer URI, falling back to perspective default.
    #[must_use]
    pub fn active_buffer(&self) -> BufferUri {
        self.layout
            .find(self.focused_pane)
            .and_then(|pane| pane.active_buffer().cloned())
            .unwrap_or_else(|| self.perspective.default_buffer())
    }

    /// Per-buffer freshness guard for the currently focused buffer.
    #[must_use]
    pub fn active_freshness(&self) -> FreshnessGuard {
        let key = self.active_buffer().to_uri_string();
        FreshnessGuard {
            observed_at_ms: self.buffer_observed_ms.get(&key).copied(),
            sse_connected: self.sse_connected,
        }
    }

    /// Project the current pure state into a renderer-agnostic [`WorkspaceFrame`].
    #[must_use]
    pub fn frame(&self) -> WorkspaceFrame {
        let primary_kind = self.selection.primary.as_ref().map(|r| r.kind);
        let multi = self.selection.selected.len() >= 2;
        let which_key = self
            .pending_chord
            .map(which_key_entries)
            .unwrap_or_default();
        let pane_header_px = if self.viewport_tier == ViewportTier::Phone {
            TOUCH_TARGET_PX
        } else {
            PANE_HEADER_PX
        };
        WorkspaceFrame {
            perspective: self.perspective,
            viewport_tier: self.viewport_tier,
            mode: self.mode,
            focused_pane: self.focused_pane,
            zoomed_pane: self.zoomed_pane,
            // Active focus NEVER uses BrandIdentity (PMS 179 red stays strictly on Kiln mark).
            active_focus_border: TokenRole::FocusInkBorder,
            active_focus_header: TokenRole::FocusHeaderInverted,
            top_strip_px: TOP_STRIP_PX,
            modeline_px: MODELINE_PX,
            pane_header_px,
            modeline_hints: modeline_hints(primary_kind, multi),
            which_key,
            dock: self.dock.clone(),
            overlay: self.overlay.clone(),
        }
    }

    /// Deterministic pure state transition `(self, event, now_ms) -> Vec<UiEffect>`.
    pub fn reduce(&mut self, event: UiEvent, now_ms: u64) -> Vec<UiEffect> {
        self.last_blocked = None;
        match event {
            UiEvent::Resize { width_px } => {
                let next_tier = ViewportTier::from_width_px(width_px);
                if next_tier != self.viewport_tier {
                    self.viewport_tier = next_tier;
                    self.layout = SplitTree::for_perspective(self.perspective, next_tier);
                    self.focused_pane = PaneId(1);
                    self.zoomed_pane = None;
                }
                Vec::new()
            }
            UiEvent::SetOperatorAuthority { can_operate } => {
                self.can_operate = can_operate;
                Vec::new()
            }
            UiEvent::SseStatusChanged { connected } => {
                self.sse_connected = connected;
                Vec::new()
            }
            UiEvent::BufferObserved {
                uri,
                observed_at_ms,
            } => {
                self.buffer_observed_ms
                    .insert(uri.to_uri_string(), observed_at_ms);
                Vec::new()
            }
            UiEvent::CursorChanged {
                target,
                run_outcome,
            } => {
                self.selection.set_primary(target);
                self.cursor_run_outcome = run_outcome;
                Vec::new()
            }
            UiEvent::MutationSettled {
                request_id,
                target_id,
                fence,
                status,
            } => self.handle_mutation_settled(request_id, target_id, fence, status),
            UiEvent::RestoreFromQuery { query } => {
                if let Ok((perspective, uri)) = BufferUri::from_query_string(&query) {
                    self.perspective = perspective;
                    self.layout = SplitTree::for_perspective(perspective, self.viewport_tier);
                    self.focused_pane = PaneId(1);
                    if let Some(pane) = self.layout.find_mut(PaneId(1)) {
                        pane.tabs = vec![uri.clone()];
                        pane.active_tab = 0;
                    }
                    self.record_jump(perspective, uri);
                }
                Vec::new()
            }
            UiEvent::Key(key_event) => self.handle_key(key_event, now_ms),
        }
    }

    fn handle_mutation_settled(
        &mut self,
        request_id: String,
        target_id: String,
        fence: Option<u64>,
        status: ReceiptStatus,
    ) -> Vec<UiEffect> {
        let mut effects = Vec::new();
        if let Some(pending) = &self.pending_mutation
            && pending.request_id == request_id
        {
            self.pending_mutation = None;
        }
        let succeeded = matches!(status, ReceiptStatus::Succeeded);
        self.receipt_ledger.record(BatchReceipt {
            target_id: target_id.clone(),
            fence,
            status,
        });
        if succeeded {
            // Only advance cursor on confirmed HTTP 2xx — never optimistically!
            effects.push(UiEffect::AdvanceCursorAfterSuccess { target_id });
        } else {
            // Keep failed/unknown receipts visible in Layer-1 dock.
            self.dock = Layer1Dock::BatchReceipts {
                ledger: self.receipt_ledger.clone(),
            };
        }
        effects
    }

    fn handle_key(&mut self, event: KeyEvent, now_ms: u64) -> Vec<UiEffect> {
        // Unconditional Esc exits Input/Command modes and closes overlays/docks.
        if event.key == Key::Esc {
            self.save_active_draft();
            self.mode = Mode::Normal;
            self.pending_chord = None;
            self.dock = Layer1Dock::None;
            self.overlay = Layer2Overlay::None;
            return Vec::new();
        }

        match self.mode {
            Mode::Input => self.handle_input_mode_key(event, now_ms),
            Mode::Command => self.handle_command_mode_key(event, now_ms),
            Mode::Normal => {
                let cmd = decode_normal_key(event, self.pending_chord);
                self.execute_command(cmd, now_ms)
            }
        }
    }

    fn handle_input_mode_key(&mut self, event: KeyEvent, now_ms: u64) -> Vec<UiEffect> {
        match event.key {
            Key::Char(ch) if !event.mods.ctrl && !event.mods.alt && !event.mods.meta => {
                if let Layer1Dock::SteeringBar { draft, .. } = &mut self.dock {
                    draft.push(ch);
                }
                Vec::new()
            }
            Key::Backspace => {
                if let Layer1Dock::SteeringBar { draft, .. } = &mut self.dock {
                    draft.pop();
                }
                Vec::new()
            }
            Key::Enter if !event.mods.shift => {
                let (target, payload) = match &self.dock {
                    Layer1Dock::SteeringBar { target, draft } => {
                        let trimmed = draft.trim().to_string();
                        if trimmed.is_empty() {
                            return Vec::new();
                        }
                        (target.clone(), trimmed)
                    }
                    _ => return Vec::new(),
                };
                let (verb, target_ref, draft_key) = match target {
                    SteeringMode::PromptAgent { target_id, .. } => (
                        Verb::PromptAgent,
                        TypedRef::new(ObjectKind::Attempt, target_id.clone(), None),
                        target_id,
                    ),
                    SteeringMode::AnswerDecision { decision_id, .. } => (
                        Verb::AnswerDecision,
                        TypedRef::new(ObjectKind::Decision, decision_id.clone(), None),
                        decision_id,
                    ),
                    SteeringMode::CommentReview { change_id } => (
                        Verb::CommentReview,
                        TypedRef::new(ObjectKind::JjChange, change_id.clone(), None),
                        change_id,
                    ),
                };
                if let Err(reason) = self.guard_mutation(now_ms, verb, false) {
                    return self.block(reason);
                }
                self.drafts.remove(&draft_key);
                self.mode = Mode::Normal;
                self.dock = Layer1Dock::None;
                let req_id = self.allocate_request_id(verb, &target_ref);
                vec![UiEffect::DispatchMutation {
                    request_id: req_id,
                    verb,
                    targets: vec![target_ref],
                    payload: Some(payload),
                }]
            }
            _ => Vec::new(),
        }
    }

    fn handle_command_mode_key(&mut self, event: KeyEvent, now_ms: u64) -> Vec<UiEffect> {
        // Tier-2b inline confirm bar: `y` or `Enter` confirms!
        if let Layer1Dock::Tier2bConfirm {
            verb, target_ids, ..
        } = &self.dock
            && matches!(event.key, Key::Char('y') | Key::Enter)
        {
            let verb = *verb;
            let targets: Vec<TypedRef> = self
                .selection
                .active_targets()
                .into_iter()
                .filter(|t| target_ids.iter().any(|id| id == &t.id))
                .collect();
            if let Err(reason) = self.guard_mutation(now_ms, verb, targets.len() >= 2) {
                return self.block(reason);
            }
            self.mode = Mode::Normal;
            self.dock = Layer1Dock::None;
            if targets.len() >= 2 {
                self.receipt_ledger.reset(verb);
            }
            let first = targets.first().cloned();
            let req_id = match first {
                Some(ref t) => self.allocate_request_id(verb, t),
                None => return self.block(MutationBlockReason::MissingFenceOrIdentity),
            };
            return vec![UiEffect::DispatchMutation {
                request_id: req_id,
                verb,
                targets,
                payload: None,
            }];
        }

        // Tier-3 destructive overlay: requires non-empty reason + not stale-shifted + Enter.
        if let Layer2Overlay::Tier3DestructiveGuard {
            verb,
            target_id,
            fence,
            reason,
            stale_shifted,
            ..
        } = &mut self.overlay
        {
            match event.key {
                Key::Char(ch) if !event.mods.ctrl && !event.mods.alt && !event.mods.meta => {
                    reason.push(ch);
                    return Vec::new();
                }
                Key::Backspace => {
                    reason.pop();
                    return Vec::new();
                }
                Key::Enter => {
                    let trimmed = reason.trim().to_string();
                    if trimmed.is_empty() || *stale_shifted {
                        return Vec::new();
                    }
                    let verb = *verb;
                    let target = TypedRef::new(ObjectKind::Attempt, target_id.clone(), *fence);
                    if let Err(block_reason) = self.guard_mutation(now_ms, verb, false) {
                        return self.block(block_reason);
                    }
                    self.mode = Mode::Normal;
                    self.overlay = Layer2Overlay::None;
                    let req_id = self.allocate_request_id(verb, &target);
                    return vec![UiEffect::DispatchMutation {
                        request_id: req_id,
                        verb,
                        targets: vec![target],
                        payload: Some(trimmed),
                    }];
                }
                _ => return Vec::new(),
            }
        }

        // Command/Filter/Picker text editing.
        match event.key {
            Key::Char(ch) if !event.mods.ctrl && !event.mods.alt && !event.mods.meta => {
                match &mut self.dock {
                    Layer1Dock::CommandBar { input } => input.push(ch),
                    Layer1Dock::FilterBar { query } => query.push(ch),
                    _ => {}
                }
                if let Layer2Overlay::FuzzyPicker { query } = &mut self.overlay {
                    query.push(ch);
                }
                Vec::new()
            }
            Key::Backspace => {
                match &mut self.dock {
                    Layer1Dock::CommandBar { input } => {
                        input.pop();
                    }
                    Layer1Dock::FilterBar { query } => {
                        query.pop();
                    }
                    _ => {}
                }
                if let Layer2Overlay::FuzzyPicker { query } = &mut self.overlay {
                    query.pop();
                }
                Vec::new()
            }
            Key::Enter => {
                self.mode = Mode::Normal;
                self.dock = Layer1Dock::None;
                self.overlay = Layer2Overlay::None;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn execute_command(&mut self, cmd: KeyCommand, now_ms: u64) -> Vec<UiEffect> {
        match cmd {
            KeyCommand::ReturnNormal => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                self.mode = Mode::Normal;
                Vec::new()
            }
            KeyCommand::StartChord(chord) => {
                self.pending_chord = Some(chord);
                self.overlay = Layer2Overlay::WhichKey { chord };
                Vec::new()
            }
            KeyCommand::SwitchPerspective(perspective) => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                self.perspective = perspective;
                self.layout = SplitTree::for_perspective(perspective, self.viewport_tier);
                self.focused_pane = PaneId(1);
                self.zoomed_pane = None;
                let buf = self.active_buffer();
                self.record_jump(perspective, buf.clone());
                vec![UiEffect::SyncUrl {
                    query: buf.to_query_string(perspective),
                }]
            }
            KeyCommand::MoveDown => {
                self.pending_chord = None;
                vec![UiEffect::MoveCursor { delta: 1 }]
            }
            KeyCommand::MoveUp => {
                self.pending_chord = None;
                vec![UiEffect::MoveCursor { delta: -1 }]
            }
            KeyCommand::MoveLeft | KeyCommand::MoveRight | KeyCommand::NoOp => {
                self.pending_chord = None;
                Vec::new()
            }
            KeyCommand::InspectAdjacent => {
                self.pending_chord = None;
                match self.selection.primary.clone() {
                    Some(target) => vec![UiEffect::InspectTarget {
                        target,
                        new_split: false,
                    }],
                    None => Vec::new(),
                }
            }
            KeyCommand::InspectNewSplit => {
                self.pending_chord = None;
                match self.selection.primary.clone() {
                    Some(target) => vec![UiEffect::InspectTarget {
                        target,
                        new_split: true,
                    }],
                    None => Vec::new(),
                }
            }
            KeyCommand::JumpBack => {
                self.pending_chord = None;
                if self.jump_index > 0 {
                    self.jump_index -= 1;
                    return self.apply_jump_entry();
                }
                Vec::new()
            }
            KeyCommand::JumpForward => {
                self.pending_chord = None;
                if self.jump_index + 1 < self.jump_history.len() {
                    self.jump_index += 1;
                    return self.apply_jump_entry();
                }
                Vec::new()
            }
            KeyCommand::Window(action) => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                self.apply_window_action(action);
                Vec::new()
            }
            KeyCommand::SemanticPrev(target) => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                vec![UiEffect::SemanticNavigate {
                    direction: -1,
                    target,
                }]
            }
            KeyCommand::SemanticNext(target) => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                vec![UiEffect::SemanticNavigate {
                    direction: 1,
                    target,
                }]
            }
            KeyCommand::GraphJump(target) => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                vec![UiEffect::GraphCrossJump {
                    from: self.selection.primary.clone(),
                    target,
                }]
            }
            KeyCommand::SelectToggle => {
                self.pending_chord = None;
                if let Some(primary) = self.selection.primary.clone() {
                    self.selection.toggle(primary);
                }
                Vec::new()
            }
            KeyCommand::SelectRange | KeyCommand::SelectAllFiltered => {
                self.pending_chord = None;
                Vec::new()
            }
            KeyCommand::SelectCollapse => {
                self.pending_chord = None;
                self.selection.collapse_to_primary();
                Vec::new()
            }
            KeyCommand::OpenFuzzyPicker => {
                self.pending_chord = None;
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::FuzzyPicker {
                    query: String::new(),
                };
                Vec::new()
            }
            KeyCommand::OpenBufferList => {
                self.pending_chord = None;
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::FuzzyPicker {
                    query: String::from("buf:"),
                };
                Vec::new()
            }
            KeyCommand::OpenActionMenu => {
                self.pending_chord = None;
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::ActionMenu;
                Vec::new()
            }
            KeyCommand::OpenHelp => {
                self.pending_chord = None;
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::Help;
                Vec::new()
            }
            KeyCommand::OpenExBar => {
                self.pending_chord = None;
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::None;
                self.dock = Layer1Dock::CommandBar {
                    input: String::new(),
                };
                Vec::new()
            }
            KeyCommand::OpenFilterBar => {
                self.pending_chord = None;
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::None;
                self.dock = Layer1Dock::FilterBar {
                    query: String::new(),
                };
                Vec::new()
            }
            KeyCommand::ConfirmYes => Vec::new(),
            KeyCommand::InvokeVerb(raw_verb) => {
                self.pending_chord = None;
                self.overlay = Layer2Overlay::None;
                self.invoke_verb(raw_verb, now_ms)
            }
        }
    }

    fn invoke_verb(&mut self, raw_verb: Verb, now_ms: u64) -> Vec<UiEffect> {
        let verb = self.contextualize_verb(raw_verb);

        // Read-only yank verbs never require freshness or operator authority.
        if matches!(
            verb,
            Verb::YankIdentity | Verb::YankUrl | Verb::YankAttach | Verb::YankCli
        ) {
            return vec![UiEffect::YankToClipboard {
                verb,
                targets: self.selection.active_targets(),
            }];
        }

        // Enforce strict multi-select whitelist inside the pure reducer.
        let tier = match self.selection.admit_verb(verb) {
            Ok(t) => t,
            Err(reason) => return self.block(reason),
        };

        // Fail-closed: Unknown runs MUST be reconciled before rerun.
        if let Some(outcome) = self.cursor_run_outcome
            && let Err(reason) = check_run_verb(outcome, verb)
        {
            return self.block(reason);
        }

        if verb.is_read_only() {
            let targets = self.selection.active_targets();
            if verb == Verb::OpenSplits {
                return targets
                    .into_iter()
                    .map(|target| UiEffect::InspectTarget {
                        target,
                        new_split: true,
                    })
                    .collect();
            }
            if verb == Verb::PipeContext {
                if let Some(first) = targets.first() {
                    self.mode = Mode::Input;
                    self.dock = Layer1Dock::SteeringBar {
                        target: SteeringMode::PromptAgent {
                            machine: String::from("active"),
                            target_id: first.id.clone(),
                        },
                        draft: format!("Context: {:?} {}", first.kind, first.id),
                    };
                }
                return Vec::new();
            }
            return Vec::new();
        }

        // Every mutating verb passes authority + 30s per-buffer freshness + no in-flight lock.
        let targets = self.selection.active_targets();
        if let Err(reason) = self.guard_mutation(now_ms, verb, targets.len() >= 2) {
            return self.block(reason);
        }

        match tier {
            SafetyTier::Tier1Instant => {
                let Some(first) = targets.first().cloned() else {
                    return self.block(MutationBlockReason::MissingFenceOrIdentity);
                };
                let req_id = self.allocate_request_id(verb, &first);
                vec![UiEffect::DispatchMutation {
                    request_id: req_id,
                    verb,
                    targets,
                    payload: None,
                }]
            }
            SafetyTier::Tier2aInput => {
                let Some(first) = targets.first() else {
                    return self.block(MutationBlockReason::MissingFenceOrIdentity);
                };
                let (steering_mode, initial_draft) = match (verb, first.kind) {
                    (Verb::NudgeAgentPrefill, _) => (
                        SteeringMode::PromptAgent {
                            machine: String::from("focused"),
                            target_id: first.id.clone(),
                        },
                        DEFAULT_NUDGE_TEMPLATE.to_string(),
                    ),
                    (_, ObjectKind::Decision) => (
                        SteeringMode::AnswerDecision {
                            decision_id: first.id.clone(),
                            target_id: first.id.clone(),
                        },
                        self.drafts.get(&first.id).cloned().unwrap_or_default(),
                    ),
                    (_, ObjectKind::JjChange) => (
                        SteeringMode::CommentReview {
                            change_id: first.id.clone(),
                        },
                        self.drafts.get(&first.id).cloned().unwrap_or_default(),
                    ),
                    _ => (
                        SteeringMode::PromptAgent {
                            machine: String::from("focused"),
                            target_id: first.id.clone(),
                        },
                        self.drafts.get(&first.id).cloned().unwrap_or_default(),
                    ),
                };
                self.mode = Mode::Input;
                self.dock = Layer1Dock::SteeringBar {
                    target: steering_mode,
                    draft: initial_draft,
                };
                Vec::new()
            }
            SafetyTier::Tier2bInlineConfirm => {
                let target_ids: Vec<String> = targets.iter().map(|t| t.id.clone()).collect();
                let summary = format!("{verb:?} ({} target(s))? [y/Enter] [Esc]", target_ids.len());
                self.mode = Mode::Command;
                self.dock = Layer1Dock::Tier2bConfirm {
                    verb,
                    summary,
                    target_ids,
                };
                Vec::new()
            }
            SafetyTier::Tier3DestructiveOverlay => {
                let Some(first) = targets.first() else {
                    return self.block(MutationBlockReason::MissingFenceOrIdentity);
                };
                let cli_command = match verb {
                    Verb::ForceReleaseClaim => format!(
                        "kiln-fleet release --claim {} --fence {}",
                        first.id,
                        first.fence_or_version.unwrap_or(0)
                    ),
                    Verb::CancelAttempt => format!("kiln-fleet attempts cancel {}", first.id),
                    Verb::CancelRun => format!("kiln ci cancel {}", first.id),
                    _ => format!("kiln {:?}", verb),
                };
                self.mode = Mode::Command;
                self.overlay = Layer2Overlay::Tier3DestructiveGuard {
                    verb,
                    target_id: first.id.clone(),
                    fence: first.fence_or_version,
                    cli_command,
                    reason: String::new(),
                    stale_shifted: false,
                };
                Vec::new()
            }
        }
    }

    fn contextualize_verb(&self, raw: Verb) -> Verb {
        let primary_kind = self.selection.primary.as_ref().map(|r| r.kind);
        match (raw, primary_kind) {
            (Verb::RenewClaim30m, Some(ObjectKind::CiRun)) => Verb::RerunExact,
            (Verb::PromptAgent, Some(ObjectKind::Decision)) => Verb::AnswerDecision,
            (Verb::CancelAttempt, Some(ObjectKind::Claim)) => Verb::ForceReleaseClaim,
            (Verb::CancelAttempt, Some(ObjectKind::CiRun)) => Verb::CancelRun,
            (Verb::CancelAttempt, Some(ObjectKind::Repository)) => Verb::RemoveRepoFromCatalog,
            _ => raw,
        }
    }

    fn guard_mutation(
        &self,
        now_ms: u64,
        verb: Verb,
        is_multi: bool,
    ) -> Result<(), MutationBlockReason> {
        if !self.can_operate {
            return Err(MutationBlockReason::ReadOnlyPrincipal);
        }
        if let Some(pending) = &self.pending_mutation {
            return Err(MutationBlockReason::MutationAlreadyInFlight {
                request_id: pending.request_id.clone(),
            });
        }
        if is_multi && !verb.allowed_on_multi_select() {
            return Err(MutationBlockReason::SingleTargetOnly {
                verb,
                count: self.selection.active_count(),
            });
        }
        self.active_freshness().check_fresh(now_ms)?;
        Ok(())
    }

    fn allocate_request_id(&mut self, verb: Verb, target: &TypedRef) -> String {
        let seq = self.next_request_seq;
        self.next_request_seq = self.next_request_seq.saturating_add(1);
        let req_id = format!("ui-req-{seq}");
        self.pending_mutation = Some(PendingMutation {
            request_id: req_id.clone(),
            verb,
            target_id: target.id.clone(),
            expected_fence: target.fence_or_version,
        });
        req_id
    }

    fn block(&mut self, reason: MutationBlockReason) -> Vec<UiEffect> {
        self.last_blocked = Some(reason.clone());
        vec![UiEffect::ShowBlockedNotice { reason }]
    }

    fn save_active_draft(&mut self) {
        if let Layer1Dock::SteeringBar { target, draft } = &self.dock {
            let key = match target {
                SteeringMode::PromptAgent { target_id, .. }
                | SteeringMode::AnswerDecision {
                    decision_id: target_id,
                    ..
                }
                | SteeringMode::CommentReview {
                    change_id: target_id,
                } => target_id.clone(),
            };
            if !draft.is_empty() {
                self.drafts.insert(key, draft.clone());
            }
        }
    }

    fn apply_window_action(&mut self, action: WindowAction) {
        let ids = self.layout.pane_ids();
        match action {
            WindowAction::SplitVertical => {
                let new_id = PaneId(self.next_pane_id);
                if self.layout.split_pane(
                    self.focused_pane,
                    Axis::Vertical,
                    new_id,
                    self.active_buffer(),
                ) {
                    self.next_pane_id = self.next_pane_id.saturating_add(1);
                    self.focused_pane = new_id;
                    self.zoomed_pane = None;
                }
            }
            WindowAction::SplitHorizontal => {
                let new_id = PaneId(self.next_pane_id);
                if self.layout.split_pane(
                    self.focused_pane,
                    Axis::Horizontal,
                    new_id,
                    self.active_buffer(),
                ) {
                    self.next_pane_id = self.next_pane_id.saturating_add(1);
                    self.focused_pane = new_id;
                    self.zoomed_pane = None;
                }
            }
            WindowAction::FocusNext | WindowAction::FocusRight | WindowAction::FocusDown => {
                if let Some(pos) = ids.iter().position(|id| *id == self.focused_pane)
                    && let Some(next) = ids.get((pos + 1) % ids.len().max(1)).copied()
                {
                    self.focused_pane = next;
                }
            }
            WindowAction::FocusPrev | WindowAction::FocusLeft | WindowAction::FocusUp => {
                if let Some(pos) = ids.iter().position(|id| *id == self.focused_pane) {
                    let prev_idx = if pos == 0 {
                        ids.len().saturating_sub(1)
                    } else {
                        pos - 1
                    };
                    if let Some(prev) = ids.get(prev_idx).copied() {
                        self.focused_pane = prev;
                    }
                }
            }
            WindowAction::FocusIndex(idx) => {
                if idx > 0
                    && let Some(target) = ids.get(usize::from(idx - 1)).copied()
                {
                    self.focused_pane = target;
                }
            }
            WindowAction::ToggleZoom => {
                self.zoomed_pane = if self.zoomed_pane == Some(self.focused_pane) {
                    None
                } else {
                    Some(self.focused_pane)
                };
            }
            WindowAction::ClosePane => {
                if self.layout.close_pane(self.focused_pane) {
                    self.zoomed_pane = None;
                    if let Some(first) = self.layout.pane_ids().first().copied() {
                        self.focused_pane = first;
                    }
                }
            }
            WindowAction::Equalize => self.layout.equalize(),
        }
    }

    fn record_jump(&mut self, perspective: Perspective, uri: BufferUri) {
        if self.jump_index + 1 < self.jump_history.len() {
            self.jump_history.truncate(self.jump_index + 1);
        }
        if self.jump_history.len() >= MAX_JUMP_HISTORY {
            self.jump_history.remove(0);
        }
        self.jump_history.push((perspective, uri));
        self.jump_index = self.jump_history.len().saturating_sub(1);
    }

    fn apply_jump_entry(&mut self) -> Vec<UiEffect> {
        let Some((perspective, uri)) = self.jump_history.get(self.jump_index).cloned() else {
            return Vec::new();
        };
        self.perspective = perspective;
        self.layout = SplitTree::for_perspective(perspective, self.viewport_tier);
        self.focused_pane = PaneId(1);
        if let Some(pane) = self.layout.find_mut(PaneId(1)) {
            pane.tabs = vec![uri.clone()];
            pane.active_tab = 0;
        }
        vec![UiEffect::SyncUrl {
            query: uri.to_query_string(perspective),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uri::UriError;

    fn sample_blake3(ch: char) -> String {
        core::iter::repeat_n(ch, 64).collect()
    }

    fn fresh_operator_state(width_px: u16, now_ms: u64) -> WorkspaceState {
        let mut state = WorkspaceState::new(width_px);
        state.reduce(UiEvent::SetOperatorAuthority { can_operate: true }, now_ms);
        state.reduce(UiEvent::SseStatusChanged { connected: true }, now_ms);
        let active = state.active_buffer();
        state.reduce(
            UiEvent::BufferObserved {
                uri: active,
                observed_at_ms: now_ms,
            },
            now_ms,
        );
        state
    }

    #[test]
    fn buffer_uri_round_trips_and_rejects_malformed_inputs() {
        let id = sample_blake3('a');
        let claim = BufferUri::Claim {
            id: id.clone(),
            fence: 1103,
        };
        let query = claim.to_query_string(Perspective::CiFleet);
        let parsed = BufferUri::from_query_string(&query);
        assert_eq!(parsed, Ok((Perspective::CiFleet, claim)));

        // Negative: invalid BLAKE3, zero fence, control chars, unknown scheme.
        assert_eq!(
            BufferUri::parse_uri("kiln://attempt/not-hex"),
            Err(UriError::InvalidBlake3)
        );
        assert_eq!(
            BufferUri::parse_uri(&format!("kiln://claim/{id}@0")),
            Err(UriError::InvalidFence)
        );
        assert_eq!(
            BufferUri::parse_uri("http://example.com/triage"),
            Err(UriError::UnknownSchemeOrKind)
        );
        assert_eq!(
            BufferUri::parse_uri("kiln://machine/bad\nname"),
            Err(UriError::InvalidCharacters)
        );
    }

    #[test]
    fn responsive_tiers_adapt_for_phone_laptop_and_wide() {
        let phone = WorkspaceState::new(390);
        assert_eq!(phone.viewport_tier, ViewportTier::Phone);
        assert_eq!(phone.layout.pane_count(), 1);
        assert_eq!(phone.frame().pane_header_px, TOUCH_TARGET_PX);

        let laptop = WorkspaceState::new(1280);
        assert_eq!(laptop.viewport_tier, ViewportTier::Laptop);
        assert_eq!(laptop.layout.pane_count(), 2);
        assert_eq!(laptop.frame().pane_header_px, PANE_HEADER_PX);

        let wide = WorkspaceState::new(1920);
        assert_eq!(wide.viewport_tier, ViewportTier::Wide);
        assert_eq!(wide.layout.pane_count(), 4);
        assert_eq!(wide.frame().active_focus_border, TokenRole::FocusInkBorder);
        assert_ne!(wide.frame().active_focus_border, TokenRole::BrandIdentity);
    }

    #[test]
    fn single_claim_renew_is_tier1_instant_and_multi_claim_promotes_to_tier2b() {
        let now = 100_000;
        let mut state = fresh_operator_state(1280, now);
        let c1 = TypedRef::new(ObjectKind::Claim, sample_blake3('a'), Some(1101));
        let c2 = TypedRef::new(ObjectKind::Claim, sample_blake3('b'), Some(1102));

        // Single claim `r` fires Tier-1 instant mutation (0 confirms) and locks pending_mutation.
        state.reduce(
            UiEvent::CursorChanged {
                target: Some(c1.clone()),
                run_outcome: None,
            },
            now,
        );
        let eff = state.reduce(UiEvent::Key(KeyEvent::char('r')), now);
        assert!(matches!(
            eff.first(),
            Some(UiEffect::DispatchMutation {
                verb: Verb::RenewClaim30m,
                ..
            })
        ));
        assert!(state.pending_mutation.is_some());

        // Settle the first request so lock clears.
        let req_id = state
            .pending_mutation
            .as_ref()
            .map(|p| p.request_id.clone())
            .unwrap_or_default();
        let settled = state.reduce(
            UiEvent::MutationSettled {
                request_id: req_id,
                target_id: c1.id.clone(),
                fence: Some(1101),
                status: ReceiptStatus::Succeeded,
            },
            now + 50,
        );
        assert!(matches!(
            settled.first(),
            Some(UiEffect::AdvanceCursorAfterSuccess { .. })
        ));

        // Multi-select 2 claims -> `r` auto-promotes to Tier-2b inline confirm!
        state.selection.toggle(c1);
        state.selection.toggle(c2);
        let eff2 = state.reduce(UiEvent::Key(KeyEvent::char('r')), now + 100);
        assert!(eff2.is_empty());
        assert_eq!(state.mode, Mode::Command);
        assert!(matches!(
            state.dock,
            Layer1Dock::Tier2bConfirm {
                verb: Verb::RenewClaim30m,
                ..
            }
        ));
    }

    #[test]
    fn multi_select_forbids_blind_prompt_broadcast_and_bulk_cancel() {
        let now = 100_000;
        let mut state = fresh_operator_state(1280, now);
        let a1 = TypedRef::new(ObjectKind::Attempt, sample_blake3('1'), Some(1));
        let a2 = TypedRef::new(ObjectKind::Attempt, sample_blake3('2'), Some(2));
        state.selection.toggle(a1);
        state.selection.toggle(a2);

        // Blind multi-agent prompt broadcast (`i`) MUST be rejected fail-closed!
        let prompt_eff = state.reduce(UiEvent::Key(KeyEvent::char('i')), now);
        assert!(matches!(
            prompt_eff.first(),
            Some(UiEffect::ShowBlockedNotice {
                reason: MutationBlockReason::SingleTargetOnly {
                    verb: Verb::PromptAgent,
                    count: 2
                }
            })
        ));

        // Bulk cancel (`D`) MUST be rejected fail-closed!
        let cancel_eff = state.reduce(UiEvent::Key(KeyEvent::char('D')), now);
        assert!(matches!(
            cancel_eff.first(),
            Some(UiEffect::ShowBlockedNotice {
                reason: MutationBlockReason::SingleTargetOnly {
                    verb: Verb::CancelAttempt,
                    count: 2
                }
            })
        ));
    }

    #[test]
    fn nudge_prefills_steering_bar_and_esc_always_returns_to_normal() {
        let now = 100_000;
        let mut state = fresh_operator_state(1280, now);
        let a1 = TypedRef::new(ObjectKind::Attempt, sample_blake3('1'), Some(1));
        state.reduce(
            UiEvent::CursorChanged {
                target: Some(a1.clone()),
                run_outcome: None,
            },
            now,
        );

        // `n` MUST pre-fill the Steering Bar in Input mode, NEVER firing immediately.
        let eff = state.reduce(UiEvent::Key(KeyEvent::char('n')), now);
        assert!(eff.is_empty());
        assert_eq!(state.mode, Mode::Input);
        assert!(matches!(
            &state.dock,
            Layer1Dock::SteeringBar { draft, .. } if draft == DEFAULT_NUDGE_TEMPLATE
        ));

        // Esc unconditionally exits Input -> Normal and preserves the draft.
        state.reduce(UiEvent::Key(KeyEvent::bare(Key::Esc)), now + 10);
        assert_eq!(state.mode, Mode::Normal);
        assert_eq!(state.dock, Layer1Dock::None);
        assert_eq!(
            state.drafts.get(&a1.id).map(String::as_str),
            Some(DEFAULT_NUDGE_TEMPLATE)
        );
    }

    #[test]
    fn unknown_run_blocks_rerun_and_stale_snapshot_blocks_all_mutations() {
        let now = 100_000;
        let mut state = fresh_operator_state(1280, now);
        let run = TypedRef::new(ObjectKind::CiRun, sample_blake3('c'), None);
        state.reduce(
            UiEvent::CursorChanged {
                target: Some(run),
                run_outcome: Some(RunOutcome::Unknown),
            },
            now,
        );

        // `r` on an Unknown run MUST be rejected with UnknownRunMustReconcile.
        let rerun_eff = state.reduce(UiEvent::Key(KeyEvent::char('r')), now);
        assert!(matches!(
            rerun_eff.first(),
            Some(UiEffect::ShowBlockedNotice {
                reason: MutationBlockReason::UnknownRunMustReconcile
            })
        ));

        // `p` (ProbeReconcile) IS allowed while fresh.
        let probe_eff = state.reduce(UiEvent::Key(KeyEvent::char('p')), now);
        assert!(matches!(
            probe_eff.first(),
            Some(UiEffect::DispatchMutation {
                verb: Verb::ProbeReconcile,
                ..
            })
        ));

        // After 30s (CONTROL_FRESHNESS_MS), mutations fail closed with StaleSnapshot.
        state.pending_mutation = None;
        let stale_eff = state.reduce(UiEvent::Key(KeyEvent::char('p')), now + 30_001);
        assert!(matches!(
            stale_eff.first(),
            Some(UiEffect::ShowBlockedNotice {
                reason: MutationBlockReason::StaleSnapshot { .. }
            })
        ));
    }

    #[test]
    fn canonical_graph_chords_resolve_without_collisions() {
        let now = 100_000;
        let mut state = fresh_operator_state(1280, now);

        // `g c` -> Cairn, `g l` -> ClaimLease, `g s` -> Swarm, `g ]` -> CausalNext
        state.reduce(UiEvent::Key(KeyEvent::char('g')), now);
        assert_eq!(state.pending_chord, Some(PendingChord::Goto));
        assert!(!state.frame().which_key.is_empty());

        let eff_c = state.reduce(UiEvent::Key(KeyEvent::char('c')), now);
        assert_eq!(
            eff_c,
            vec![UiEffect::GraphCrossJump {
                from: None,
                target: GraphTarget::Cairn
            }]
        );

        state.reduce(UiEvent::Key(KeyEvent::char('g')), now);
        let eff_l = state.reduce(UiEvent::Key(KeyEvent::char('l')), now);
        assert_eq!(
            eff_l,
            vec![UiEffect::GraphCrossJump {
                from: None,
                target: GraphTarget::ClaimLease
            }]
        );

        state.reduce(UiEvent::Key(KeyEvent::char('g')), now);
        let eff_s = state.reduce(UiEvent::Key(KeyEvent::char('s')), now);
        assert_eq!(
            eff_s,
            vec![UiEffect::GraphCrossJump {
                from: None,
                target: GraphTarget::Swarm
            }]
        );
    }

    #[test]
    fn non_2xx_settlement_does_not_auto_advance_and_pins_receipt_ledger() {
        let now = 100_000;
        let mut state = fresh_operator_state(1280, now);
        let c1 = TypedRef::new(ObjectKind::Claim, sample_blake3('f'), Some(42));
        state.reduce(
            UiEvent::CursorChanged {
                target: Some(c1.clone()),
                run_outcome: None,
            },
            now,
        );
        state.reduce(UiEvent::Key(KeyEvent::char('r')), now);
        let req_id = state
            .pending_mutation
            .as_ref()
            .map(|p| p.request_id.clone())
            .unwrap_or_default();

        let eff = state.reduce(
            UiEvent::MutationSettled {
                request_id: req_id,
                target_id: c1.id,
                fence: Some(42),
                status: ReceiptStatus::RejectedStaleFence {
                    detail: String::from("409 fence mismatch"),
                },
            },
            now + 100,
        );
        // Must NOT emit AdvanceCursorAfterSuccess on 409!
        assert!(eff.is_empty());
        assert_eq!(state.receipt_ledger.failed_or_unknown_count(), 1);
        assert!(matches!(state.dock, Layer1Dock::BatchReceipts { .. }));
    }
}
