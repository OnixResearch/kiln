//! Finite spreadsheet value, formula, and deterministic error envelopes.

use crate::spreadsheet_address::CellAddress;
use crate::spreadsheet_address::SheetBounds;
use crate::spreadsheet_address::SpreadsheetRange;
use crate::spreadsheet_address::spreadsheet_address_admitted;
use crate::spreadsheet_address::spreadsheet_range_admit;
use crate::spreadsheet_address::spreadsheet_range_expand;

pub const SPREADSHEET_COUNT_INCREMENT: i64 = 1;
pub const SPREADSHEET_ZERO: i64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpreadsheetFormulaError {
    BadReference,
    DivisionByZero,
    ValueShapeMismatch,
    CycleRejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpreadsheetValue {
    Blank,
    Number(i64),
    Boolean(bool),
    TextTag(u64),
    Error(SpreadsheetFormulaError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpreadsheetFormula {
    Constant(SpreadsheetValue),
    Reference(CellAddress),
    RangeSum(SpreadsheetRange),
    RangeCount(SpreadsheetRange),
    RangeMin(SpreadsheetRange),
    RangeMax(SpreadsheetRange),
    Add(Box<SpreadsheetFormula>, Box<SpreadsheetFormula>),
    Divide(Box<SpreadsheetFormula>, Box<SpreadsheetFormula>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpreadsheetCellValue {
    pub address: CellAddress,
    pub value: SpreadsheetValue,
}

fn spreadsheet_range_admitted(bounds: SheetBounds, range: SpreadsheetRange) -> bool {
    spreadsheet_range_admit(bounds, range.start, range.end).is_ok()
}

// r[impl domain-primitives.spreadsheet.formula]
pub fn spreadsheet_formula_admitted(bounds: SheetBounds, formula: &SpreadsheetFormula) -> bool {
    match formula {
        SpreadsheetFormula::Constant(_) => true,
        SpreadsheetFormula::Reference(address) => spreadsheet_address_admitted(bounds, *address),
        SpreadsheetFormula::RangeSum(range)
        | SpreadsheetFormula::RangeCount(range)
        | SpreadsheetFormula::RangeMin(range)
        | SpreadsheetFormula::RangeMax(range) => spreadsheet_range_admitted(bounds, *range),
        SpreadsheetFormula::Add(left, right) | SpreadsheetFormula::Divide(left, right) => {
            spreadsheet_formula_admitted(bounds, left) && spreadsheet_formula_admitted(bounds, right)
        }
    }
}

fn spreadsheet_lookup_value(values: &[SpreadsheetCellValue], address: CellAddress) -> Option<SpreadsheetValue> {
    values.iter().find(|cell| cell.address == address).map(|cell| cell.value)
}

fn spreadsheet_number_value(value: SpreadsheetValue) -> Result<i64, SpreadsheetFormulaError> {
    match value {
        SpreadsheetValue::Number(number) => Ok(number),
        SpreadsheetValue::Error(error) => Err(error),
        SpreadsheetValue::Blank | SpreadsheetValue::Boolean(_) | SpreadsheetValue::TextTag(_) => {
            Err(SpreadsheetFormulaError::ValueShapeMismatch)
        }
    }
}

fn spreadsheet_eval_number(
    bounds: SheetBounds,
    formula: &SpreadsheetFormula,
    values: &[SpreadsheetCellValue],
) -> Result<i64, SpreadsheetFormulaError> {
    spreadsheet_number_value(spreadsheet_formula_evaluate(bounds, formula, values))
}

fn spreadsheet_range_numbers(
    range: SpreadsheetRange,
    values: &[SpreadsheetCellValue],
) -> Result<Vec<i64>, SpreadsheetFormulaError> {
    let mut numbers = Vec::new();
    for address in spreadsheet_range_expand(range) {
        let Some(value) = spreadsheet_lookup_value(values, address) else {
            return Err(SpreadsheetFormulaError::BadReference);
        };
        numbers.push(spreadsheet_number_value(value)?);
    }
    Ok(numbers)
}

fn spreadsheet_sum_range(
    range: SpreadsheetRange,
    values: &[SpreadsheetCellValue],
) -> Result<i64, SpreadsheetFormulaError> {
    let mut total = SPREADSHEET_ZERO;
    for number in spreadsheet_range_numbers(range, values)? {
        total = total.checked_add(number).ok_or(SpreadsheetFormulaError::ValueShapeMismatch)?;
    }
    Ok(total)
}

fn spreadsheet_count_range(
    range: SpreadsheetRange,
    values: &[SpreadsheetCellValue],
) -> Result<i64, SpreadsheetFormulaError> {
    let mut count = SPREADSHEET_ZERO;
    for address in spreadsheet_range_expand(range) {
        let Some(value) = spreadsheet_lookup_value(values, address) else {
            return Err(SpreadsheetFormulaError::BadReference);
        };
        if matches!(value, SpreadsheetValue::Error(_)) {
            return Err(SpreadsheetFormulaError::ValueShapeMismatch);
        }
        count = count.checked_add(SPREADSHEET_COUNT_INCREMENT).ok_or(SpreadsheetFormulaError::ValueShapeMismatch)?;
    }
    Ok(count)
}

fn spreadsheet_min_range(
    range: SpreadsheetRange,
    values: &[SpreadsheetCellValue],
) -> Result<i64, SpreadsheetFormulaError> {
    let mut numbers = spreadsheet_range_numbers(range, values)?.into_iter();
    let Some(mut best) = numbers.next() else {
        return Err(SpreadsheetFormulaError::ValueShapeMismatch);
    };
    for number in numbers {
        best = best.min(number);
    }
    Ok(best)
}

fn spreadsheet_max_range(
    range: SpreadsheetRange,
    values: &[SpreadsheetCellValue],
) -> Result<i64, SpreadsheetFormulaError> {
    let mut numbers = spreadsheet_range_numbers(range, values)?.into_iter();
    let Some(mut best) = numbers.next() else {
        return Err(SpreadsheetFormulaError::ValueShapeMismatch);
    };
    for number in numbers {
        best = best.max(number);
    }
    Ok(best)
}

// r[impl domain-primitives.spreadsheet.error]
// r[impl domain-primitives.spreadsheet.formula]
pub fn spreadsheet_formula_evaluate(
    bounds: SheetBounds,
    formula: &SpreadsheetFormula,
    values: &[SpreadsheetCellValue],
) -> SpreadsheetValue {
    if !spreadsheet_formula_admitted(bounds, formula) {
        return SpreadsheetValue::Error(SpreadsheetFormulaError::BadReference);
    }
    let result = match formula {
        SpreadsheetFormula::Constant(value) => return *value,
        SpreadsheetFormula::Reference(address) => spreadsheet_lookup_value(values, *address)
            .ok_or(SpreadsheetFormulaError::BadReference)
            .and_then(spreadsheet_number_value),
        SpreadsheetFormula::RangeSum(range) => spreadsheet_sum_range(*range, values),
        SpreadsheetFormula::RangeCount(range) => spreadsheet_count_range(*range, values),
        SpreadsheetFormula::RangeMin(range) => spreadsheet_min_range(*range, values),
        SpreadsheetFormula::RangeMax(range) => spreadsheet_max_range(*range, values),
        SpreadsheetFormula::Add(left, right) => {
            let left_number = spreadsheet_eval_number(bounds, left, values);
            let right_number = spreadsheet_eval_number(bounds, right, values);
            match (left_number, right_number) {
                (Ok(left_value), Ok(right_value)) => {
                    left_value.checked_add(right_value).ok_or(SpreadsheetFormulaError::ValueShapeMismatch)
                }
                (Err(error), _) | (_, Err(error)) => Err(error),
            }
        }
        SpreadsheetFormula::Divide(left, right) => {
            let left_number = spreadsheet_eval_number(bounds, left, values);
            let right_number = spreadsheet_eval_number(bounds, right, values);
            match (left_number, right_number) {
                (Ok(_), Ok(SPREADSHEET_ZERO)) => Err(SpreadsheetFormulaError::DivisionByZero),
                (Ok(left_value), Ok(right_value)) => Ok(left_value / right_value),
                (Err(error), _) | (_, Err(error)) => Err(error),
            }
        }
    };
    match result {
        Ok(number) => SpreadsheetValue::Number(number),
        Err(error) => SpreadsheetValue::Error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: u16 = 3;
    const COLUMNS: u16 = 3;
    const FIRST_ROW: u16 = 0;
    const FIRST_COLUMN: u16 = 0;
    const SECOND_COLUMN: u16 = 1;
    const THIRD_COLUMN: u16 = 2;
    const OUT_OF_BOUNDS_ROW: u16 = ROWS;
    const LEFT_VALUE: i64 = 6;
    const RIGHT_VALUE: i64 = 3;
    const ZERO_VALUE: i64 = 0;
    const SUM_VALUE: i64 = 9;
    const COUNT_VALUE: i64 = 2;
    const MIN_VALUE: i64 = 3;
    const MAX_VALUE: i64 = 6;
    const TEXT_TAG: u64 = 7;

    fn bounds() -> SheetBounds {
        SheetBounds {
            row_count: ROWS,
            column_count: COLUMNS,
        }
    }

    fn cell(column: u16) -> CellAddress {
        CellAddress { row: FIRST_ROW, column }
    }

    fn range() -> SpreadsheetRange {
        SpreadsheetRange {
            start: cell(FIRST_COLUMN),
            end: cell(SECOND_COLUMN),
        }
    }

    fn values() -> Vec<SpreadsheetCellValue> {
        vec![
            SpreadsheetCellValue {
                address: cell(FIRST_COLUMN),
                value: SpreadsheetValue::Number(LEFT_VALUE),
            },
            SpreadsheetCellValue {
                address: cell(SECOND_COLUMN),
                value: SpreadsheetValue::Number(RIGHT_VALUE),
            },
            SpreadsheetCellValue {
                address: cell(THIRD_COLUMN),
                value: SpreadsheetValue::Number(ZERO_VALUE),
            },
        ]
    }

    // r[verify domain-primitives.spreadsheet.formula]
    #[test]
    fn formula_admits_constants_references_ranges_arithmetic_and_aggregates() {
        assert!(spreadsheet_formula_admitted(
            bounds(),
            &SpreadsheetFormula::Constant(SpreadsheetValue::Number(LEFT_VALUE))
        ));
        assert!(spreadsheet_formula_admitted(bounds(), &SpreadsheetFormula::Reference(cell(FIRST_COLUMN))));
        assert!(spreadsheet_formula_admitted(bounds(), &SpreadsheetFormula::RangeSum(range())));
        assert!(spreadsheet_formula_admitted(
            bounds(),
            &SpreadsheetFormula::Add(
                Box::new(SpreadsheetFormula::Reference(cell(FIRST_COLUMN))),
                Box::new(SpreadsheetFormula::Reference(cell(SECOND_COLUMN)))
            )
        ));
    }

    // r[verify domain-primitives.spreadsheet.formula]
    #[test]
    fn invalid_formula_shape_is_rejected_before_evaluation() {
        let invalid_reference = SpreadsheetFormula::Reference(CellAddress {
            row: OUT_OF_BOUNDS_ROW,
            column: FIRST_COLUMN,
        });
        assert!(!spreadsheet_formula_admitted(bounds(), &invalid_reference));
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &invalid_reference, &values()),
            SpreadsheetValue::Error(SpreadsheetFormulaError::BadReference)
        );
    }

    // r[verify domain-primitives.spreadsheet.error]
    #[test]
    fn evaluation_returns_deterministic_division_by_zero_and_bad_reference_errors() {
        let div_zero = SpreadsheetFormula::Divide(
            Box::new(SpreadsheetFormula::Reference(cell(FIRST_COLUMN))),
            Box::new(SpreadsheetFormula::Reference(cell(THIRD_COLUMN))),
        );
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &div_zero, &values()),
            SpreadsheetValue::Error(SpreadsheetFormulaError::DivisionByZero)
        );
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &div_zero, &values()),
            spreadsheet_formula_evaluate(bounds(), &div_zero, &values())
        );

        let missing = SpreadsheetFormula::Reference(cell(FIRST_COLUMN));
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &missing, &[]),
            SpreadsheetValue::Error(SpreadsheetFormulaError::BadReference)
        );
    }

    // r[verify domain-primitives.spreadsheet.error]
    #[test]
    fn value_shape_mismatch_is_explicit() {
        let text_value = [SpreadsheetCellValue {
            address: cell(FIRST_COLUMN),
            value: SpreadsheetValue::TextTag(TEXT_TAG),
        }];
        let reference = SpreadsheetFormula::Reference(cell(FIRST_COLUMN));
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &reference, &text_value),
            SpreadsheetValue::Error(SpreadsheetFormulaError::ValueShapeMismatch)
        );
    }

    // r[verify domain-primitives.spreadsheet.formula]
    #[test]
    fn aggregates_use_exact_range_values() {
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &SpreadsheetFormula::RangeSum(range()), &values()),
            SpreadsheetValue::Number(SUM_VALUE)
        );
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &SpreadsheetFormula::RangeCount(range()), &values()),
            SpreadsheetValue::Number(COUNT_VALUE)
        );
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &SpreadsheetFormula::RangeMin(range()), &values()),
            SpreadsheetValue::Number(MIN_VALUE)
        );
        assert_eq!(
            spreadsheet_formula_evaluate(bounds(), &SpreadsheetFormula::RangeMax(range()), &values()),
            SpreadsheetValue::Number(MAX_VALUE)
        );
    }
}
