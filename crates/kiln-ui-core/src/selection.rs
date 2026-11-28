//! Helix-style typed `SelectionSet` (`x`/`X`/`%`/`,`) with strict batch safety gating.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::safety::{MutationBlockReason, SafetyTier, Verb};
use crate::uri::ObjectKind;

/// Maximum items admitted into a single selection set.
pub const MAX_SELECTION_ITEMS: usize = 64;

/// Exact typed reference to a Kiln object currently under cursor or selected.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TypedRef {
    pub kind: ObjectKind,
    pub id: String,
    pub fence_or_version: Option<u64>,
}

impl TypedRef {
    /// Construct a typed reference bound to an exact identity and optional fence/version.
    #[must_use]
    pub fn new(kind: ObjectKind, id: String, fence_or_version: Option<u64>) -> Self {
        Self {
            kind,
            id,
            fence_or_version,
        }
    }
}

/// Primary cursor plus optional typed multi-selection set.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectionSet {
    pub primary: Option<TypedRef>,
    pub selected: Vec<TypedRef>,
}

impl SelectionSet {
    /// Set the primary cursor without disturbing existing multi-select marks.
    pub fn set_primary(&mut self, target: Option<TypedRef>) {
        self.primary = target;
    }

    /// Toggle an item in the selection set (`x` key). Bounded at `MAX_SELECTION_ITEMS`.
    pub fn toggle(&mut self, target: TypedRef) {
        if let Some(pos) = self
            .selected
            .iter()
            .position(|item| item.kind == target.kind && item.id == target.id)
        {
            self.selected.remove(pos);
        } else if self.selected.len() < MAX_SELECTION_ITEMS {
            self.selected.push(target);
        }
    }

    /// Select all visible items matching the active filter (`%` key), bounded at `MAX_SELECTION_ITEMS`.
    pub fn select_all(&mut self, items: &[TypedRef]) {
        self.selected.clear();
        for item in items.iter().take(MAX_SELECTION_ITEMS) {
            self.selected.push(item.clone());
        }
    }

    /// Collapse multi-selection back to the primary cursor (`,` key).
    pub fn collapse_to_primary(&mut self) {
        self.selected.clear();
    }

    /// Retain only targets whose `id` is still in `keep_ids` (e.g. non-2xx batch receipts).
    pub fn retain_ids(&mut self, keep_ids: &[String]) {
        self.selected
            .retain(|item| keep_ids.iter().any(|id| id == &item.id));
    }

    /// Active targets that a verb would act upon: `selected` if non-empty, else `[primary]`.
    #[must_use]
    pub fn active_targets(&self) -> Vec<TypedRef> {
        if !self.selected.is_empty() {
            return self.selected.clone();
        }
        match &self.primary {
            Some(item) => alloc::vec![item.clone()],
            None => Vec::new(),
        }
    }

    /// Effective count of active targets.
    #[must_use]
    pub fn active_count(&self) -> usize {
        if !self.selected.is_empty() {
            self.selected.len()
        } else if self.primary.is_some() {
            1
        } else {
            0
        }
    }

    /// Validate whether `verb` may execute on the current selection set, returning its effective safety tier.
    pub fn admit_verb(&self, verb: Verb) -> Result<SafetyTier, MutationBlockReason> {
        let count = self.active_count();
        if count == 0 {
            return Err(MutationBlockReason::MissingFenceOrIdentity);
        }
        verb.effective_tier(count)
            .ok_or(MutationBlockReason::SingleTargetOnly { verb, count })
    }
}
