//! Finite spreadsheet sheet-bound, address, and range envelopes.

pub const SPREADSHEET_MIN_DIMENSION: u16 = 1;
pub const SPREADSHEET_MAX_DIMENSION: u16 = 1024;
pub const SPREADSHEET_EMPTY_COUNT: u16 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SheetBounds {
    pub row_count: u16,
    pub column_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellAddress {
    pub row: u16,
    pub column: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpreadsheetRange {
    pub start: CellAddress,
    pub end: CellAddress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpreadsheetAddressError {
    InvalidSheetBounds,
    AddressOutOfBounds,
}

// r[impl domain-primitives.spreadsheet.address]
pub fn spreadsheet_sheet_bounds_admitted(bounds: SheetBounds) -> bool {
    (SPREADSHEET_MIN_DIMENSION..=SPREADSHEET_MAX_DIMENSION).contains(&bounds.row_count)
        && (SPREADSHEET_MIN_DIMENSION..=SPREADSHEET_MAX_DIMENSION).contains(&bounds.column_count)
}

// r[impl domain-primitives.spreadsheet.address]
pub fn spreadsheet_address_admitted(bounds: SheetBounds, address: CellAddress) -> bool {
    spreadsheet_sheet_bounds_admitted(bounds) && address.row < bounds.row_count && address.column < bounds.column_count
}

fn normalize_endpoint_order(first: CellAddress, second: CellAddress) -> SpreadsheetRange {
    SpreadsheetRange {
        start: CellAddress {
            row: first.row.min(second.row),
            column: first.column.min(second.column),
        },
        end: CellAddress {
            row: first.row.max(second.row),
            column: first.column.max(second.column),
        },
    }
}

// r[impl domain-primitives.spreadsheet.range]
pub fn spreadsheet_range_admit(
    bounds: SheetBounds,
    first: CellAddress,
    second: CellAddress,
) -> Result<SpreadsheetRange, SpreadsheetAddressError> {
    if !spreadsheet_sheet_bounds_admitted(bounds) {
        return Err(SpreadsheetAddressError::InvalidSheetBounds);
    }
    if !spreadsheet_address_admitted(bounds, first) || !spreadsheet_address_admitted(bounds, second) {
        return Err(SpreadsheetAddressError::AddressOutOfBounds);
    }
    Ok(normalize_endpoint_order(first, second))
}

// r[impl domain-primitives.spreadsheet.range]
pub fn spreadsheet_range_contains(range: SpreadsheetRange, address: CellAddress) -> bool {
    range.start.row <= address.row
        && address.row <= range.end.row
        && range.start.column <= address.column
        && address.column <= range.end.column
}

// r[impl domain-primitives.spreadsheet.range]
pub fn spreadsheet_range_expand(range: SpreadsheetRange) -> Vec<CellAddress> {
    let mut cells = Vec::new();
    for row in range.start.row..=range.end.row {
        for column in range.start.column..=range.end.column {
            cells.push(CellAddress { row, column });
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: u16 = 4;
    const COLUMNS: u16 = 5;
    const FIRST_ROW: u16 = 0;
    const FIRST_COLUMN: u16 = 0;
    const SECOND_ROW: u16 = 1;
    const SECOND_COLUMN: u16 = 2;
    const OUT_OF_BOUNDS_ROW: u16 = ROWS;
    const OUT_OF_BOUNDS_COLUMN: u16 = COLUMNS;
    const EXPECTED_RANGE_CELLS: usize = 6;

    fn bounds() -> SheetBounds {
        SheetBounds {
            row_count: ROWS,
            column_count: COLUMNS,
        }
    }

    fn first_cell() -> CellAddress {
        CellAddress {
            row: FIRST_ROW,
            column: FIRST_COLUMN,
        }
    }

    fn second_cell() -> CellAddress {
        CellAddress {
            row: SECOND_ROW,
            column: SECOND_COLUMN,
        }
    }

    // r[verify domain-primitives.spreadsheet.address]
    #[test]
    fn admits_in_bounds_address_deterministically() {
        assert!(spreadsheet_address_admitted(bounds(), first_cell()));
        assert_eq!(
            spreadsheet_address_admitted(bounds(), first_cell()),
            spreadsheet_address_admitted(bounds(), first_cell())
        );
    }

    // r[verify domain-primitives.spreadsheet.address]
    #[test]
    fn rejects_out_of_bounds_address_without_admission() {
        let row_bad = CellAddress {
            row: OUT_OF_BOUNDS_ROW,
            column: FIRST_COLUMN,
        };
        let column_bad = CellAddress {
            row: FIRST_ROW,
            column: OUT_OF_BOUNDS_COLUMN,
        };
        assert!(!spreadsheet_address_admitted(bounds(), row_bad));
        assert!(!spreadsheet_address_admitted(bounds(), column_bad));
    }

    // r[verify domain-primitives.spreadsheet.range]
    #[test]
    fn reversed_range_endpoints_normalize_to_same_rectangle() {
        let forward = spreadsheet_range_admit(bounds(), first_cell(), second_cell()).expect("forward range");
        let reversed = spreadsheet_range_admit(bounds(), second_cell(), first_cell()).expect("reversed range");
        assert_eq!(forward, reversed);
        assert_eq!(spreadsheet_range_expand(forward).len(), EXPECTED_RANGE_CELLS);
    }

    // r[verify domain-primitives.spreadsheet.range]
    #[test]
    fn invalid_range_endpoint_is_rejected_before_expansion() {
        let invalid = CellAddress {
            row: OUT_OF_BOUNDS_ROW,
            column: FIRST_COLUMN,
        };
        assert_eq!(
            spreadsheet_range_admit(bounds(), first_cell(), invalid),
            Err(SpreadsheetAddressError::AddressOutOfBounds)
        );
    }
}
