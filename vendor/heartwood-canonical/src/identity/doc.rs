//! Adapted from upstream `radicle::identity::doc`: only the validated
//! `Delegates` and `Threshold` types used by canonical rules.

use std::fmt;
use std::num::NonZeroUsize;

use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::identity::Did;

/// Upstream bound on the number of delegates.
pub const MAX_DELEGATES: usize = 255;

#[derive(Debug, Error)]
#[error("invalid delegates: {0}")]
pub struct DelegatesError(&'static str);

#[derive(Debug, Error)]
#[error("invalid threshold `{0}`: {1}")]
pub struct ThresholdError(usize, &'static str);

/// A valid set of delegates for the identity document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<Did>")]
pub struct Delegates(NonEmpty<Did>);

impl AsRef<NonEmpty<Did>> for Delegates {
    fn as_ref(&self) -> &NonEmpty<Did> {
        &self.0
    }
}

impl From<Did> for Delegates {
    fn from(did: Did) -> Self {
        Self(NonEmpty::new(did))
    }
}

impl TryFrom<Vec<Did>> for Delegates {
    type Error = DelegatesError;

    fn try_from(dids: Vec<Did>) -> Result<Self, Self::Error> {
        Delegates::new(dids)
    }
}

impl IntoIterator for Delegates {
    type Item = <NonEmpty<Did> as IntoIterator>::Item;
    type IntoIter = <NonEmpty<Did> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Delegates {
    /// Construct the set by removing duplicates, rejecting an empty list and
    /// enforcing [`MAX_DELEGATES`].
    pub fn new(delegates: impl IntoIterator<Item = Did>) -> Result<Self, DelegatesError> {
        let delegates = delegates
            .into_iter()
            .try_fold(Vec::<Did>::new(), |mut dids, did| {
                if !dids.contains(&did) {
                    if dids.len() >= MAX_DELEGATES {
                        return Err(DelegatesError("number of delegates cannot exceed 255"));
                    }
                    dids.push(did);
                }
                Ok(dids)
            })?;
        NonEmpty::from_vec(delegates)
            .map(Self)
            .ok_or(DelegatesError("delegate list cannot be empty"))
    }

    /// Get the first delegate in the set.
    pub fn first(&self) -> &Did {
        self.0.first()
    }

    /// Obtain an iterator over the DIDs.
    pub fn iter(&self) -> impl Iterator<Item = &Did> {
        self.0.iter()
    }

    /// Check if the set contains the given DID.
    pub fn contains(&self, did: &Did) -> bool {
        self.0.contains(did)
    }

    /// Check if the DID is the only delegate.
    pub fn is_only(&self, did: &Did) -> bool {
        self.0.tail.is_empty() && &self.0.head == did
    }

    /// Get the number of delegates in the set.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`; the set is non-empty by construction.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl<'a> From<&'a Delegates> for &'a NonEmpty<Did> {
    fn from(ds: &'a Delegates) -> Self {
        &ds.0
    }
}

impl From<Delegates> for NonEmpty<Did> {
    fn from(ds: Delegates) -> Self {
        ds.0
    }
}

impl From<Delegates> for Vec<Did> {
    fn from(Delegates(ds): Delegates) -> Self {
        ds.into()
    }
}

/// A valid threshold for the identity document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Threshold(NonZeroUsize);

impl From<Threshold> for usize {
    fn from(Threshold(t): Threshold) -> Self {
        t.get()
    }
}

impl AsRef<NonZeroUsize> for Threshold {
    fn as_ref(&self) -> &NonZeroUsize {
        &self.0
    }
}

impl fmt::Display for Threshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Threshold {
    /// A threshold of `1`.
    pub const MIN: Threshold = Threshold(NonZeroUsize::MIN);

    /// Validate a threshold against [`MAX_DELEGATES`] and the delegate count.
    pub fn new(t: usize, delegates: &Delegates) -> Result<Self, ThresholdError> {
        if t > MAX_DELEGATES {
            Err(ThresholdError(t, "threshold cannot exceed 255"))
        } else if t > delegates.len() {
            Err(ThresholdError(
                t,
                "threshold cannot exceed number of delegates",
            ))
        } else {
            NonZeroUsize::new(t)
                .map(Self)
                .ok_or(ThresholdError(t, "threshold cannot be zero"))
        }
    }
}
