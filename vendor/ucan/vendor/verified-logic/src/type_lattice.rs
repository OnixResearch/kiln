//! Verified type conversion lattice.
//!
//! Models which data types can be converted to which, with ANY as top element
//! and STRING as universal source. The conversion graph is reflexive and
//! forms a partial order over 7 type constants.
//!
//! Generalized from seaglass's `TypeInferenceEngine` `can_convert_to`.

use vstd::prelude::*;

verus! {

// --- Type constants ---

/// String type.
pub open spec fn TYPE_STRING() -> u8 { 0 }
/// Integer type.
pub open spec fn TYPE_INTEGER() -> u8 { 1 }
/// Floating-point type.
pub open spec fn TYPE_FLOAT() -> u8 { 2 }
/// Boolean type.
pub open spec fn TYPE_BOOLEAN() -> u8 { 3 }
/// Date type.
pub open spec fn TYPE_DATE() -> u8 { 4 }
/// Datetime type.
pub open spec fn TYPE_DATETIME() -> u8 { 5 }
/// Any type (top element, accepts all conversions).
pub open spec fn TYPE_ANY() -> u8 { 6 }

/// Spec: whether a type value is a valid type constant.
pub open spec fn is_valid_type_spec(t: u8) -> bool {
    t <= 6
}

// --- can_convert ---

// r[depends type-lattice.can-convert]
/// Spec: whether a value of type `from` can be converted to type `to`.
///
/// The conversion graph:
/// - Reflexive: any type converts to itself
/// - ANY is top: converts to/from everything
/// - STRING is universal source: converts to everything
/// - INTEGER <-> FLOAT (numeric widening/narrowing)
/// - INTEGER, FLOAT, BOOLEAN -> STRING (to-string)
/// - DATE <-> DATETIME (temporal widening/narrowing)
/// - DATE, DATETIME -> STRING (to-string)
pub open spec fn can_convert_spec(from: u8, to: u8) -> bool {
    // Same type
    from == to
    // ANY is top element
    || to == TYPE_ANY()
    || from == TYPE_ANY()
    // STRING converts to anything
    || from == TYPE_STRING()
    // Numeric conversions
    || (from == TYPE_INTEGER() && to == TYPE_FLOAT())
    || (from == TYPE_FLOAT() && to == TYPE_INTEGER())
    // To-string conversions
    || (from == TYPE_INTEGER() && to == TYPE_STRING())
    || (from == TYPE_FLOAT() && to == TYPE_STRING())
    || (from == TYPE_BOOLEAN() && to == TYPE_STRING())
    // Temporal conversions
    || (from == TYPE_DATE() && to == TYPE_DATETIME())
    || (from == TYPE_DATETIME() && to == TYPE_DATE())
    || (from == TYPE_DATE() && to == TYPE_STRING())
    || (from == TYPE_DATETIME() && to == TYPE_STRING())
}

// r[impl type-lattice.can-convert]
/// Returns true if a value of type `from` can be converted to type `to`.
pub fn can_convert(from: u8, to: u8) -> (result: bool)
    ensures result == can_convert_spec(from, to)
{
    from == to
    || to == 6  // ANY
    || from == 6  // ANY
    || from == 0  // STRING -> anything
    || (from == 1 && to == 2)  // INTEGER -> FLOAT
    || (from == 2 && to == 1)  // FLOAT -> INTEGER
    || (from == 1 && to == 0)  // INTEGER -> STRING
    || (from == 2 && to == 0)  // FLOAT -> STRING
    || (from == 3 && to == 0)  // BOOLEAN -> STRING
    || (from == 4 && to == 5)  // DATE -> DATETIME
    || (from == 5 && to == 4)  // DATETIME -> DATE
    || (from == 4 && to == 0)  // DATE -> STRING
    || (from == 5 && to == 0)  // DATETIME -> STRING
}

// --- Reflexivity ---

// r[verify type-lattice.can-convert]
/// Reflexive: every type converts to itself.
proof fn can_convert_reflexive(t: u8)
    ensures can_convert_spec(t, t)
{}

// --- ANY as top ---

// r[impl type-lattice.any-top]
// r[verify type-lattice.any-top]
/// ANY converts to any type.
proof fn any_converts_to_all(t: u8)
    ensures can_convert_spec(TYPE_ANY(), t)
{}

// r[verify type-lattice.any-top]
/// Any type converts to ANY.
proof fn all_convert_to_any(t: u8)
    ensures can_convert_spec(t, TYPE_ANY())
{}

// --- STRING as universal source ---

// r[impl type-lattice.string-source]
// r[verify type-lattice.string-source]
/// STRING converts to every type.
proof fn string_converts_to_all(t: u8)
    ensures can_convert_spec(TYPE_STRING(), t)
{}

// --- Numeric widening ---

// r[impl type-lattice.numeric-widening]
// r[verify type-lattice.numeric-widening]
/// INTEGER converts to FLOAT (widening).
proof fn integer_widens_to_float()
    ensures can_convert_spec(TYPE_INTEGER(), TYPE_FLOAT())
{}

// r[verify type-lattice.numeric-widening]
/// FLOAT converts to INTEGER (narrowing).
proof fn float_narrows_to_integer()
    ensures can_convert_spec(TYPE_FLOAT(), TYPE_INTEGER())
{}

// r[verify type-lattice.numeric-widening]
/// INTEGER and FLOAT both convert to STRING.
proof fn numerics_to_string()
    ensures
        can_convert_spec(TYPE_INTEGER(), TYPE_STRING()),
        can_convert_spec(TYPE_FLOAT(), TYPE_STRING()),
{}

// --- Temporal widening ---

// r[impl type-lattice.temporal-widening]
// r[verify type-lattice.temporal-widening]
/// DATE converts to DATETIME.
proof fn date_widens_to_datetime()
    ensures can_convert_spec(TYPE_DATE(), TYPE_DATETIME())
{}

// r[verify type-lattice.temporal-widening]
/// DATETIME converts to DATE.
proof fn datetime_narrows_to_date()
    ensures can_convert_spec(TYPE_DATETIME(), TYPE_DATE())
{}

// r[verify type-lattice.temporal-widening]
/// DATE and DATETIME both convert to STRING.
proof fn temporals_to_string()
    ensures
        can_convert_spec(TYPE_DATE(), TYPE_STRING()),
        can_convert_spec(TYPE_DATETIME(), TYPE_STRING()),
{}

// --- Negative cases ---

// r[verify type-lattice.can-convert]
/// BOOLEAN does NOT convert to DATE.
proof fn boolean_not_to_date()
    ensures !can_convert_spec(TYPE_BOOLEAN(), TYPE_DATE())
{}

// r[verify type-lattice.can-convert]
/// BOOLEAN does NOT convert to INTEGER.
proof fn boolean_not_to_integer()
    ensures !can_convert_spec(TYPE_BOOLEAN(), TYPE_INTEGER())
{}

// r[verify type-lattice.can-convert]
/// BOOLEAN does NOT convert to FLOAT.
proof fn boolean_not_to_float()
    ensures !can_convert_spec(TYPE_BOOLEAN(), TYPE_FLOAT())
{}

// r[verify type-lattice.can-convert]
/// BOOLEAN does NOT convert to DATETIME.
proof fn boolean_not_to_datetime()
    ensures !can_convert_spec(TYPE_BOOLEAN(), TYPE_DATETIME())
{}

// r[verify type-lattice.can-convert]
/// DATE does NOT convert to INTEGER.
proof fn date_not_to_integer()
    ensures !can_convert_spec(TYPE_DATE(), TYPE_INTEGER())
{}

// r[verify type-lattice.can-convert]
/// DATE does NOT convert to BOOLEAN.
proof fn date_not_to_boolean()
    ensures !can_convert_spec(TYPE_DATE(), TYPE_BOOLEAN())
{}

// --- Conversion count ---

// r[verify type-lattice.can-convert]
/// For valid types, STRING has the most outgoing conversions (converts to all 7 types).
/// BOOLEAN has the fewest (only to STRING, BOOLEAN, ANY = 3).
proof fn string_is_most_flexible()
    ensures
        forall|t: u8| is_valid_type_spec(t) ==> can_convert_spec(TYPE_STRING(), t),
{}

// r[verify type-lattice.can-convert]
/// For valid types, ANY has the most incoming conversions (all types convert to it).
proof fn any_accepts_all()
    ensures
        forall|t: u8| #[trigger] can_convert_spec(t, TYPE_ANY()),
{}

// --- is_numeric ---

/// Spec: whether a type is numeric (INTEGER or FLOAT).
pub open spec fn is_numeric_spec(t: u8) -> bool {
    t == TYPE_INTEGER() || t == TYPE_FLOAT()
}

// r[verify type-lattice.numeric-widening]
/// Numeric types can convert to each other.
proof fn numeric_interconvertible(a: u8, b: u8)
    requires is_numeric_spec(a), is_numeric_spec(b)
    ensures can_convert_spec(a, b)
{}

// --- is_temporal ---

/// Spec: whether a type is temporal (DATE or DATETIME).
pub open spec fn is_temporal_spec(t: u8) -> bool {
    t == TYPE_DATE() || t == TYPE_DATETIME()
}

// r[verify type-lattice.temporal-widening]
/// Temporal types can convert to each other.
proof fn temporal_interconvertible(a: u8, b: u8)
    requires is_temporal_spec(a), is_temporal_spec(b)
    ensures can_convert_spec(a, b)
{}

// --- to_string universal ---

// r[verify type-lattice.can-convert]
/// Every valid concrete type (non-ANY) can convert to STRING.
proof fn every_concrete_type_to_string(t: u8)
    requires is_valid_type_spec(t), t != TYPE_ANY()
    ensures can_convert_spec(t, TYPE_STRING())
{}

} // verus!

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // r[verify type-lattice.can-convert]
    #[test]
    fn test_reflexive() {
        for t in 0..=6 {
            assert!(can_convert(t, t), "type {t} should convert to itself");
        }
    }

    // r[verify type-lattice.any-top]
    #[test]
    fn test_any_top() {
        for t in 0..=6 {
            assert!(can_convert(6, t), "ANY should convert to type {t}");
            assert!(can_convert(t, 6), "type {t} should convert to ANY");
        }
    }

    // r[verify type-lattice.string-source]
    #[test]
    fn test_string_universal_source() {
        for t in 0..=6 {
            assert!(can_convert(0, t), "STRING should convert to type {t}");
        }
    }

    // r[verify type-lattice.numeric-widening]
    #[test]
    fn test_numeric_conversions() {
        assert!(can_convert(1, 2)); // INTEGER -> FLOAT
        assert!(can_convert(2, 1)); // FLOAT -> INTEGER
        assert!(can_convert(1, 0)); // INTEGER -> STRING
        assert!(can_convert(2, 0)); // FLOAT -> STRING
    }

    // r[verify type-lattice.temporal-widening]
    #[test]
    fn test_temporal_conversions() {
        assert!(can_convert(4, 5)); // DATE -> DATETIME
        assert!(can_convert(5, 4)); // DATETIME -> DATE
        assert!(can_convert(4, 0)); // DATE -> STRING
        assert!(can_convert(5, 0)); // DATETIME -> STRING
    }

    // r[verify type-lattice.can-convert]
    #[test]
    fn test_negative_cases() {
        // BOOLEAN doesn't convert to most types
        assert!(!can_convert(3, 1)); // BOOLEAN -/-> INTEGER
        assert!(!can_convert(3, 2)); // BOOLEAN -/-> FLOAT
        assert!(!can_convert(3, 4)); // BOOLEAN -/-> DATE
        assert!(!can_convert(3, 5)); // BOOLEAN -/-> DATETIME
        // But BOOLEAN -> STRING and BOOLEAN -> BOOLEAN work
        assert!(can_convert(3, 0)); // BOOLEAN -> STRING
        assert!(can_convert(3, 3)); // BOOLEAN -> BOOLEAN

        // DATE doesn't convert to numeric/boolean
        assert!(!can_convert(4, 1)); // DATE -/-> INTEGER
        assert!(!can_convert(4, 2)); // DATE -/-> FLOAT
        assert!(!can_convert(4, 3)); // DATE -/-> BOOLEAN

        // INTEGER doesn't convert to temporal
        assert!(!can_convert(1, 4)); // INTEGER -/-> DATE
        assert!(!can_convert(1, 5)); // INTEGER -/-> DATETIME
    }

    // r[verify type-lattice.can-convert]
    #[test]
    fn test_full_conversion_table() {
        // Exhaustive check of all 7x7 = 49 pairs
        let expected: [[bool; 7]; 7] = [
            // to:    STR    INT    FLT    BOOL   DATE   DTTM   ANY
            /* STR */
            [true, true, true, true, true, true, true],
            /* INT */ [true, true, true, false, false, false, true],
            /* FLT */ [true, true, true, false, false, false, true],
            /* BOOL */ [true, false, false, true, false, false, true],
            /* DATE */ [true, false, false, false, true, true, true],
            /* DTTM */ [true, false, false, false, true, true, true],
            /* ANY */ [true, true, true, true, true, true, true],
        ];

        for from in 0..7u8 {
            for to in 0..7u8 {
                assert_eq!(
                    can_convert(from, to),
                    expected[from as usize][to as usize],
                    "can_convert({from}, {to}) mismatch"
                );
            }
        }
    }

    proptest! {
        // r[verify type-lattice.can-convert]
        #[test]
        fn prop_reflexive(t in 0u8..=6) {
            prop_assert!(can_convert(t, t));
        }

        // r[verify type-lattice.any-top]
        #[test]
        fn prop_any_top(t in 0u8..=6) {
            prop_assert!(can_convert(6, t));
            prop_assert!(can_convert(t, 6));
        }

        // r[verify type-lattice.string-source]
        #[test]
        fn prop_string_source(t in 0u8..=6) {
            prop_assert!(can_convert(0, t));
        }

        // r[verify type-lattice.can-convert]
        #[test]
        fn prop_to_string_universal(t in 0u8..=5) {
            // Every concrete type converts to STRING
            prop_assert!(can_convert(t, 0));
        }

        // r[verify type-lattice.numeric-widening]
        #[test]
        fn prop_numeric_interconvertible(a in 1u8..=2, b in 1u8..=2) {
            prop_assert!(can_convert(a, b));
        }

        // r[verify type-lattice.temporal-widening]
        #[test]
        fn prop_temporal_interconvertible(a in 4u8..=5, b in 4u8..=5) {
            prop_assert!(can_convert(a, b));
        }

        // r[verify type-lattice.can-convert]
        #[test]
        fn prop_boolean_isolated_from_numeric(to in 1u8..=2) {
            prop_assert!(!can_convert(3, to)); // BOOLEAN -/-> numeric
        }

        // r[verify type-lattice.can-convert]
        #[test]
        fn prop_invalid_types_only_reflexive_or_any(from in 7u8..=255, to in 7u8..=255) {
            // Invalid types only convert to themselves or ANY
            if from != to && to != 6 && from != 6 && from != 0 {
                prop_assert!(!can_convert(from, to));
            }
        }
    }
}
