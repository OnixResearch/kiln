//! Symbolic references, which link neither to nor from protected references.
//! The prototypical example is `HEAD → refs/heads/main`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::git::fmt::Qualified;
use crate::git::fmt::RefStr;
use crate::git::fmt::RefString;

use super::protect::Unprotected;

/// A type alias for a [`RefString`] that has yet to be validated into a
/// a symbolic reference name.
pub type RawName = RefString;

/// A type alias for a [`RefString`] that has yet to be validated into a
/// symbolic reference target.
pub type RawTarget = RefString;

/// The target of a symbolic reference.
///
/// A target is either:
/// - [`Direct`](Target::Direct): a concrete qualified reference
///   (e.g. `refs/heads/main`).
/// - [`Symbolic`](Target::Symbolic): another symbolic reference name
///   (e.g. `MAIN`) that must itself resolve through the chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A concrete qualified reference — the end of a chain.
    Direct(Direct),
    /// Another symbolic reference name — an intermediate link.
    Symbolic(Symbolic),
}

impl AsRef<RefStr> for Target {
    fn as_ref(&self) -> &RefStr {
        match self {
            Target::Direct(direct) => direct.0.as_ref(),
            Target::Symbolic(symbolic) => symbolic.0.as_ref(),
        }
    }
}

/// A concrete qualified reference — the end of a chain.
// `Unprotected` has `super` visibility, so `Direct` is used to allow `Target`
// to be `pub`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Direct(Unprotected<Qualified<'static>>);

impl Direct {
    fn to_ref_string(&self) -> Unprotected<RefString> {
        self.0.to_ref_string()
    }
}

impl PartialEq<RefString> for Direct {
    fn eq(&self, other: &RefString) -> bool {
        **self.0.as_ref() == **other
    }
}

impl AsRef<Qualified<'static>> for Direct {
    fn as_ref(&self) -> &Qualified<'static> {
        self.0.as_ref()
    }
}

/// A concrete qualified reference — the end of a chain.
// `Unprotected` has `super` visibility, so `Symbolic` is used to allow `Target`
// to be `pub`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbolic(Unprotected<RefString>);

impl AsRef<RefString> for Symbolic {
    fn as_ref(&self) -> &RefString {
        self.0.as_ref()
    }
}

impl Target {
    /// Returns the underlying reference as a `&RefStr`.
    pub fn as_refstr(&self) -> &RefStr {
        match self {
            Target::Direct(q) => q.as_ref().as_ref(),
            Target::Symbolic(s) => s.as_ref().as_ref(),
        }
    }

    fn direct(d: Unprotected<Qualified<'static>>) -> Self {
        Self::Direct(Direct(d))
    }

    fn symbolic(s: Unprotected<RefString>) -> Self {
        Self::Symbolic(Symbolic(s))
    }

    /// Classify an [`Unprotected<RefString>`] as either
    /// [`Direct`](Target::Direct) or [`Symbolic`](Target::Symbolic)
    /// based on whether it is [`Qualified`].
    ///
    /// The [`Unprotected`] proof is preserved in the resulting variant.
    fn classify(s: Unprotected<RefString>) -> Self {
        match Qualified::from_refstr(s.as_ref()) {
            // Safety: the Qualified is derived from an Unprotected string,
            // so it is also unprotected.
            Some(q) => Target::direct(
                Unprotected::new(q.to_owned())
                    .expect("qualified derived from unprotected is unprotected"),
            ),
            None => Target::symbolic(s),
        }
    }
}

impl Serialize for Target {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_refstr().as_str())
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_refstr().as_str())
    }
}

/// Names of symbolic references are unprotected references.
/// Requiring [`Unprotected`] makes symbolic references that link *from*
/// protected references unrepresentable.
pub(super) type Name = Unprotected<RefString>;

impl std::cmp::Ord for Name {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl std::cmp::PartialOrd for Name {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Maintains an acyclic set of symbolic references.
/// Note that dangling references are not detected.
///
/// Internally, targets are stored as [`Target`], which distinguishes
/// direct (qualified) targets from symbolic (intermediate) ones. This
/// means resolution and cycle-checking can pattern-match on the variant
/// rather than re-parsing the string.
///
/// # Deserialization Order
///
/// Deserialization validates entries in iteration order via
/// [`TryFrom<IndexMap>`]. This means deserialization only succeeds if
/// symbolic reference whose target is another symbolic
/// reference appears *after* that target in the JSON.
/// For example,
/// `{"MAIN": "refs/heads/master", "HEAD": "MAIN"}` is valid, but
/// `{"HEAD": "MAIN", "MAIN": "refs/heads/master"}` is not.
///
/// To ensure that serialization and deserialization are inverses,
/// we must ensure that insertion order and iteration order are the same.
/// We do this by using [`IndexMap`] internally, which preserves this property,
/// and enabling the feature `preserve_order` of the `serde_json` crate.
///
/// Normally, JSON objects are unordered (see [RFC 8259, Sec. 4]).
/// Any compatible (de-)serializer must preserve key order.
///
/// [RFC 8259, Sec. 4]: <https://datatracker.ietf.org/doc/html/rfc8259#section-4>
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "IndexMap<Name, Unprotected<RefString>>")]
pub struct SymbolicRefs(IndexMap<Name, Target>);

/// Read-only access.
impl SymbolicRefs {
    /// Returns an iterator over all contained symbolic references, as pairs
    /// of their name and [`Target`].
    pub fn iter(&self) -> impl Iterator<Item = (&RawName, &Target)> {
        self.0.iter().map(|(name, target)| (name.as_ref(), target))
    }

    /// Returns an iterator over all contained symbolic references that
    /// resolve to a direct (qualified) target. The yielded target is the
    /// final [`Qualified`] reference after chasing through any intermediate
    /// symbolic references.
    pub fn iter_resolved(&self) -> impl Iterator<Item = (&RawName, &Qualified<'static>)> {
        self.0.keys().filter_map(|name| {
            self.resolve(name.as_ref())
                .map(|target| (name.as_ref(), target))
        })
    }

    /// Resolve a name through the chain of symbolic references until a
    /// [`Target::Direct`] target is reached. Returns `None` if the
    /// name is not in the map or if the chain dangles (ends at a
    /// [`Target::Symbolic`] whose name is not a key).
    fn resolve(&self, name: &RefString) -> Option<&Qualified<'static>> {
        let mut current = self.0.get(name)?;
        loop {
            match current {
                Target::Direct(q) => return Some(q.as_ref()),
                Target::Symbolic(s) => current = self.0.get(s.as_ref())?,
            }
        }
    }

    /// Returns `true` if the set of symbolic references is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Utilities for handling of `HEAD`.
impl SymbolicRefs {
    /// Construct [`SymbolicRefs`] for the single symbolic reference `HEAD`
    /// targeting `/refs/heads/<branch_name>`.
    // This exists to encapsulate [`super::protect::Unprotected`].
    pub fn head(branch_name: &RefString) -> Self {
        let mut result = Self::default();
        result
            .try_insert_unprotected(
                Unprotected::head().to_owned(),
                Unprotected::branch(branch_name).to_ref_string(),
            )
            .expect("not creating cycle");
        result
    }

    /// Convenience method to get the resolved target of the `HEAD` reference.
    /// Returns the final [`Qualified`] reference after chasing the chain.
    /// See also [`SymbolicRefs::head`].
    pub fn resolve_head(&self) -> Option<&Qualified<'static>> {
        self.resolve(Unprotected::head().as_ref())
    }
}

#[derive(Debug, Error)]
pub enum InsertionError {
    #[error("inserting symbolic reference '{name} → {target}' would create a cycle")]
    Cyclic { name: RawName, target: RawName },

    #[error(
        "inserting symbolic reference '{name} → {target}' would result in a symbolic reference targeting an unqualified reference"
    )]
    TargetNotQualified { name: RawName, target: RawName },
}

/// Mutability.
impl SymbolicRefs {
    /// Insert a symbolic reference from the given `name` to the given `target`.
    ///
    /// Internally, this classifies the `target` into the [`Target`] and
    /// delegates the insertion.
    pub(super) fn try_insert_unprotected(
        &mut self,
        name: Name,
        target: Unprotected<RefString>,
    ) -> Result<(), InsertionError> {
        self.insert(name, Target::classify(target))
    }

    /// Check whether `name` is reachable from `start` by chasing through
    /// the map. Used to detect cycles before insertion.
    ///
    /// Unlike [`resolve`](SymbolicRefs::resolve), this chases through
    /// *all* entries regardless of whether the target is [`Direct`](Target::Direct)
    /// or [`Symbolic`](Target::Symbolic), since a qualified ref string can
    /// also be a key in the map and thus part of a cycle.
    fn is_reachable_from(&self, start: &RefString, name: &Name) -> bool {
        let name = name.as_ref();
        if start == name {
            return true;
        }
        let mut current: &RefStr = start;
        loop {
            match self.0.get(current) {
                None => return false,
                Some(target) => {
                    let next = target.as_refstr();
                    if *next == **name {
                        return true;
                    }
                    current = next;
                }
            }
        }
    }

    /// Try to insert a symbolic reference.
    /// Errors if `name` or `target` is protected (see [`super::protect`]) or would
    /// cause infinite recursion (e.g. `A → B` and `B → A`).
    ///
    /// # Panics
    ///
    /// If `name` or `target` is not unprotected.
    #[cfg(test)]
    fn try_insert(&mut self, name: RawName, target: RawTarget) -> Result<(), InsertionError> {
        self.try_insert_unprotected(
            Unprotected::new(name).expect("name is unprotected"),
            Unprotected::new(target).expect("target is unprotected"),
        )
    }

    /// Consume `other` by iteratively inserting into self.
    pub fn combine(&mut self, other: SymbolicRefs) -> Result<(), InsertionError> {
        for (name, target) in other.0 {
            self.insert(name, target)?;
        }
        Ok(())
    }

    /// Insert a symbolic reference from the given `name` to the given `target`.
    ///
    /// The targets in the map can change their classification from
    /// [`Target::Direct`] to [`Target::Symbolic`], if the new insertion of the
    /// `name` or `target` matches an existing key or existing entries.
    ///
    /// # Errors
    ///
    /// The `target` reference must either:
    /// - be a direct [`Qualified`] reference, or
    /// - resolve to a direct [`Qualified`] reference, if it is a keyed entry in [`SymbolicRefs`].
    ///
    /// If neither of these is satisfied then an
    /// [`InsertionError::TargetNotQualified`] error is returned.
    ///
    /// If the `name` and `target` end up in a cycle, e.g., `a → b → a`, then an
    /// [`InsertionError::Cyclic`] error is returned.
    fn insert(&mut self, name: Name, mut target: Target) -> Result<(), InsertionError> {
        let target_str = match &target {
            Target::Direct(q) => q.as_ref().to_ref_string(),
            Target::Symbolic(s) => s.as_ref().clone(),
        };

        if self.is_reachable_from(&target_str, &name) {
            return Err(InsertionError::Cyclic {
                name: name.into_inner(),
                target: target_str,
            });
        }

        // A [`Target::Direct`] whose string is already a key is actually
        // an intermediate link — downgrade to [`Target::Symbolic`].
        if let Target::Direct(q) = &target
            && self.0.contains_key(&q.as_ref().to_ref_string())
        {
            target = Target::symbolic(q.to_ref_string());
        }

        if let Target::Symbolic(s) = &target
            && self.resolve(s.as_ref()).is_none()
        {
            return Err(InsertionError::TargetNotQualified {
                name: name.into_inner(),
                target: target_str,
            });
        }

        self.reclassify_targets(&name);
        self.0.insert(name, target);
        Ok(())
    }

    /// When a new key is inserted, any existing [`Target::Direct`] whose
    /// qualified string matches the new key is no longer terminal — it is
    /// now an intermediate link and must be reclassified as [`Target::Symbolic`].
    fn reclassify_targets(&mut self, new_key: &Name) {
        let key = new_key.as_ref();
        for existing in self.0.values_mut() {
            if let Target::Direct(q) = existing
                && q == key
            {
                *existing = Target::symbolic(q.to_ref_string());
            }
        }
    }
}

impl TryFrom<IndexMap<Name, Unprotected<RefString>>> for SymbolicRefs {
    type Error = InsertionError;

    fn try_from(map: IndexMap<Name, Unprotected<RefString>>) -> Result<Self, Self::Error> {
        map.into_iter()
            .try_fold(Self::default(), |mut result, (name, target)| {
                result.try_insert_unprotected(name, target)?;
                Ok(result)
            })
    }
}
