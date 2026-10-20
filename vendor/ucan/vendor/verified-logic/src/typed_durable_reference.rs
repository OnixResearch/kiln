//! Finite typed durable reference admission primitives.
//!
//! This module models the structural admission side of Unison-style durable
//! typed values. Durable references carry opaque finite value/type/schema/code
//! identities; unpersist requests compare exact type identity and require the
//! declared dependency closure to be available; schema/type migration is admitted
//! only through an explicit finite conversion witness. No storage I/O, codec,
//! typechecker, hash-strength, or runtime durability correctness is claimed.
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TypedDurableReference {
    pub reference_hash: u64,
    pub value_hash: u64,
    pub type_hash: u64,
    pub schema_version: u64,
    pub codec_hash: u64,
    pub closure_hash: u64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClosureCatalog {
    pub available_closures: Vec<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UnpersistRequest {
    pub expected_type_hash: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConversionWitness {
    pub old_reference_hash: u64,
    pub old_type_hash: u64,
    pub old_schema_version: u64,
    pub new_reference_hash: u64,
    pub new_type_hash: u64,
    pub new_schema_version: u64,
    pub converter_closure_hash: u64,
}

// r[depends domain-primitives.typed-durable-reference.envelope]
pub open spec fn finite_reference_metadata_spec(reference: TypedDurableReference) -> bool {
    reference.reference_hash > 0
        && reference.value_hash > 0
        && reference.type_hash > 0
        && reference.codec_hash > 0
        && reference.closure_hash > 0
}

// r[depends domain-primitives.typed-durable-reference.envelope]
// r[depends domain-primitives.typed-durable-reference.unpersist-type-match]
pub open spec fn closure_available_spec(available: Seq<u64>, closure_hash: u64) -> bool {
    exists|i: int| 0 <= i < available.len() && available[i] == closure_hash
}

// r[depends domain-primitives.typed-durable-reference.envelope]
pub open spec fn typed_reference_admitted_spec(reference: TypedDurableReference, catalog: ClosureCatalog) -> bool {
    finite_reference_metadata_spec(reference)
        && closure_available_spec(catalog.available_closures@, reference.closure_hash)
}

// r[depends domain-primitives.typed-durable-reference.envelope]
pub open spec fn missing_reference_closure_spec(reference: TypedDurableReference, catalog: ClosureCatalog) -> Option<u64> {
    if closure_available_spec(catalog.available_closures@, reference.closure_hash) {
        Option::None
    } else {
        Option::Some(reference.closure_hash)
    }
}

// r[depends domain-primitives.typed-durable-reference.unpersist-type-match]
pub open spec fn unpersist_type_matches_spec(reference: TypedDurableReference, request: UnpersistRequest) -> bool {
    reference.type_hash == request.expected_type_hash
}

// r[depends domain-primitives.typed-durable-reference.unpersist-type-match]
pub open spec fn unpersist_admitted_spec(
    reference: TypedDurableReference,
    request: UnpersistRequest,
    catalog: ClosureCatalog,
) -> bool {
    typed_reference_admitted_spec(reference, catalog)
        && unpersist_type_matches_spec(reference, request)
}

// r[depends domain-primitives.typed-durable-reference.migration-boundary]
pub open spec fn conversion_witness_matches_spec(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
    witness: ConversionWitness,
) -> bool {
    witness.old_reference_hash == old_reference.reference_hash
        && witness.old_type_hash == old_reference.type_hash
        && witness.old_schema_version == old_reference.schema_version
        && witness.new_reference_hash == new_reference.reference_hash
        && witness.new_type_hash == new_reference.type_hash
        && witness.new_schema_version == new_reference.schema_version
        && witness.converter_closure_hash > 0
}

// r[depends domain-primitives.typed-durable-reference.migration-boundary]
pub open spec fn migration_admitted_spec(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
    witness: ConversionWitness,
    catalog: ClosureCatalog,
) -> bool {
    typed_reference_admitted_spec(old_reference, catalog)
        && typed_reference_admitted_spec(new_reference, catalog)
        && conversion_witness_matches_spec(old_reference, new_reference, witness)
        && closure_available_spec(catalog.available_closures@, witness.converter_closure_hash)
}

// r[depends domain-primitives.typed-durable-reference.migration-boundary]
pub open spec fn implicit_migration_admitted_spec(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
) -> bool {
    old_reference.type_hash == new_reference.type_hash
        && old_reference.schema_version == new_reference.schema_version
}

// r[impl domain-primitives.typed-durable-reference.envelope]
pub fn finite_reference_metadata(reference: TypedDurableReference) -> (result: bool)
    ensures result == finite_reference_metadata_spec(reference)
{
    reference.reference_hash > 0
        && reference.value_hash > 0
        && reference.type_hash > 0
        && reference.codec_hash > 0
        && reference.closure_hash > 0
}

// r[impl domain-primitives.typed-durable-reference.envelope]
// r[impl domain-primitives.typed-durable-reference.unpersist-type-match]
pub fn closure_available(available: &Vec<u64>, closure_hash: u64) -> (result: bool)
    ensures result == closure_available_spec(available@, closure_hash)
{
    let mut i: usize = 0;
    while i < available.len()
        invariant
            0 <= i <= available.len(),
            forall|k: int| 0 <= k < i ==> available@[k] != closure_hash,
        decreases available.len() - i,
    {
        if available[i] == closure_hash {
            assert(closure_available_spec(available@, closure_hash));
            return true;
        }
        i = i + 1;
    }
    assert forall|k: int| 0 <= k < available@.len() implies available@[k] != closure_hash by {
        assert(0 <= k < i);
    }
    assert(!closure_available_spec(available@, closure_hash));
    false
}

// r[impl domain-primitives.typed-durable-reference.envelope]
pub fn typed_reference_admitted(reference: TypedDurableReference, catalog: &ClosureCatalog) -> (result: bool)
    ensures result == typed_reference_admitted_spec(reference, *catalog)
{
    finite_reference_metadata(reference) && closure_available(&catalog.available_closures, reference.closure_hash)
}

// r[impl domain-primitives.typed-durable-reference.envelope]
pub fn missing_reference_closure(reference: TypedDurableReference, catalog: &ClosureCatalog) -> (result: Option<u64>)
    ensures result == missing_reference_closure_spec(reference, *catalog)
{
    if closure_available(&catalog.available_closures, reference.closure_hash) {
        None
    } else {
        Some(reference.closure_hash)
    }
}

// r[impl domain-primitives.typed-durable-reference.unpersist-type-match]
pub fn unpersist_type_matches(reference: TypedDurableReference, request: UnpersistRequest) -> (result: bool)
    ensures result == unpersist_type_matches_spec(reference, request)
{
    reference.type_hash == request.expected_type_hash
}

// r[impl domain-primitives.typed-durable-reference.unpersist-type-match]
pub fn unpersist_admitted(
    reference: TypedDurableReference,
    request: UnpersistRequest,
    catalog: &ClosureCatalog,
) -> (result: bool)
    ensures result == unpersist_admitted_spec(reference, request, *catalog)
{
    typed_reference_admitted(reference, catalog) && unpersist_type_matches(reference, request)
}

// r[impl domain-primitives.typed-durable-reference.migration-boundary]
pub fn conversion_witness_matches(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
    witness: ConversionWitness,
) -> (result: bool)
    ensures result == conversion_witness_matches_spec(old_reference, new_reference, witness)
{
    witness.old_reference_hash == old_reference.reference_hash
        && witness.old_type_hash == old_reference.type_hash
        && witness.old_schema_version == old_reference.schema_version
        && witness.new_reference_hash == new_reference.reference_hash
        && witness.new_type_hash == new_reference.type_hash
        && witness.new_schema_version == new_reference.schema_version
        && witness.converter_closure_hash > 0
}

// r[impl domain-primitives.typed-durable-reference.migration-boundary]
pub fn migration_admitted(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
    witness: ConversionWitness,
    catalog: &ClosureCatalog,
) -> (result: bool)
    ensures result == migration_admitted_spec(old_reference, new_reference, witness, *catalog)
{
    typed_reference_admitted(old_reference, catalog)
        && typed_reference_admitted(new_reference, catalog)
        && conversion_witness_matches(old_reference, new_reference, witness)
        && closure_available(&catalog.available_closures, witness.converter_closure_hash)
}

// r[impl domain-primitives.typed-durable-reference.migration-boundary]
pub fn implicit_migration_admitted(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
) -> (result: bool)
    ensures result == implicit_migration_admitted_spec(old_reference, new_reference)
{
    old_reference.type_hash == new_reference.type_hash
        && old_reference.schema_version == new_reference.schema_version
}

// r[verify domain-primitives.typed-durable-reference.unpersist-type-match]
pub proof fn unpersist_requires_exact_type(
    reference: TypedDurableReference,
    request: UnpersistRequest,
    catalog: ClosureCatalog,
)
    requires unpersist_admitted_spec(reference, request, catalog)
    ensures reference.type_hash == request.expected_type_hash
{
}

// r[verify domain-primitives.typed-durable-reference.envelope]
pub proof fn admitted_reference_has_available_closure(
    reference: TypedDurableReference,
    catalog: ClosureCatalog,
)
    requires typed_reference_admitted_spec(reference, catalog)
    ensures closure_available_spec(catalog.available_closures@, reference.closure_hash)
{
}

// r[verify domain-primitives.typed-durable-reference.migration-boundary]
pub proof fn migration_requires_explicit_witness_match(
    old_reference: TypedDurableReference,
    new_reference: TypedDurableReference,
    witness: ConversionWitness,
    catalog: ClosureCatalog,
)
    requires migration_admitted_spec(old_reference, new_reference, witness, catalog)
    ensures conversion_witness_matches_spec(old_reference, new_reference, witness)
{
}

} // verus!

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(type_hash: u64, schema_version: u64, closure_hash: u64) -> TypedDurableReference {
        TypedDurableReference {
            reference_hash: 10 + type_hash + schema_version,
            value_hash: 20 + type_hash,
            type_hash,
            schema_version,
            codec_hash: 30,
            closure_hash,
        }
    }

    #[test]
    fn admits_well_formed_reference_and_reports_missing_closure() {
        let catalog = ClosureCatalog {
            available_closures: vec![7, 9],
        };
        let admitted = reference(100, 1, 7);
        let missing = reference(100, 1, 8);

        assert!(typed_reference_admitted(admitted, &catalog));
        assert_eq!(missing_reference_closure(admitted, &catalog), None);
        assert!(!typed_reference_admitted(missing, &catalog));
        assert_eq!(missing_reference_closure(missing, &catalog), Some(8));
    }

    #[test]
    fn rejects_inconsistent_zero_metadata() {
        let catalog = ClosureCatalog {
            available_closures: vec![7],
        };
        let mut invalid = reference(100, 1, 7);
        invalid.type_hash = 0;

        assert!(!finite_reference_metadata(invalid));
        assert!(!typed_reference_admitted(invalid, &catalog));
    }

    #[test]
    fn unpersist_requires_type_match_and_available_closure() {
        let catalog = ClosureCatalog {
            available_closures: vec![7],
        };
        let unavailable = ClosureCatalog {
            available_closures: vec![9],
        };
        let durable = reference(100, 1, 7);

        assert!(unpersist_admitted(
            durable,
            UnpersistRequest {
                expected_type_hash: 100
            },
            &catalog,
        ));
        assert!(!unpersist_admitted(
            durable,
            UnpersistRequest {
                expected_type_hash: 101
            },
            &catalog,
        ));
        assert!(!unpersist_admitted(
            durable,
            UnpersistRequest {
                expected_type_hash: 100
            },
            &unavailable,
        ));
    }

    #[test]
    fn migration_requires_explicit_matching_witness() {
        let catalog = ClosureCatalog {
            available_closures: vec![7, 8, 99],
        };
        let old_reference = reference(100, 1, 7);
        let new_reference = reference(200, 2, 8);
        let witness = ConversionWitness {
            old_reference_hash: old_reference.reference_hash,
            old_type_hash: old_reference.type_hash,
            old_schema_version: old_reference.schema_version,
            new_reference_hash: new_reference.reference_hash,
            new_type_hash: new_reference.type_hash,
            new_schema_version: new_reference.schema_version,
            converter_closure_hash: 99,
        };
        let mut wrong_witness = witness;
        wrong_witness.new_type_hash = 201;

        assert!(migration_admitted(old_reference, new_reference, witness, &catalog));
        assert!(!migration_admitted(old_reference, new_reference, wrong_witness, &catalog));
        assert!(!implicit_migration_admitted(old_reference, new_reference));
        assert!(implicit_migration_admitted(old_reference, reference(100, 1, 8)));
    }
}
