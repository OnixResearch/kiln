//! Spreadsheet recalculation order, dirty propagation, and admission composition.

#![allow(clippy::struct_excessive_bools)]

use crate::spreadsheet_address::CellAddress;
use crate::spreadsheet_dependency::SpreadsheetDependencyEdge;
use crate::spreadsheet_dependency::SpreadsheetFormulaCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpreadsheetAdmissionChecks {
    pub address_admitted: bool,
    pub range_admitted: bool,
    pub formula_admitted: bool,
    pub dependency_extracted: bool,
    pub cycle_rejected: bool,
    pub recalc_order_admitted: bool,
    pub dirty_propagation_admitted: bool,
    pub error_envelope_deterministic: bool,
}

fn push_unique(cells: &mut Vec<CellAddress>, address: CellAddress) {
    if !cells.contains(&address) {
        cells.push(address);
    }
}

fn formula_cell_exists(cells: &[SpreadsheetFormulaCell], address: CellAddress) -> bool {
    cells.iter().any(|cell| cell.address == address)
}

fn order_position(order: &[CellAddress], address: CellAddress) -> Option<usize> {
    order.iter().position(|candidate| *candidate == address)
}

fn order_has_no_duplicates(order: &[CellAddress]) -> bool {
    let mut seen = Vec::new();
    for address in order {
        if seen.contains(address) {
            return false;
        }
        seen.push(*address);
    }
    true
}

// r[impl domain-primitives.spreadsheet.recalc]
pub fn spreadsheet_recalc_order_admitted(
    cells: &[SpreadsheetFormulaCell],
    edges: &[SpreadsheetDependencyEdge],
    order: &[CellAddress],
) -> bool {
    if !order_has_no_duplicates(order) || !order.iter().all(|address| formula_cell_exists(cells, *address)) {
        return false;
    }

    for edge in edges {
        let dependent_position = order_position(order, edge.formula_cell);
        let precedent_position = order_position(order, edge.precedent);
        if dependent_position.is_some() && formula_cell_exists(cells, edge.precedent) && precedent_position.is_none() {
            return false;
        }
        if let (Some(dependent), Some(precedent)) = (dependent_position, precedent_position) {
            if precedent >= dependent {
                return false;
            }
        }
    }
    true
}

// r[impl domain-primitives.spreadsheet.dirty-propagation]
pub fn spreadsheet_dirty_cells(edges: &[SpreadsheetDependencyEdge], changed_cells: &[CellAddress]) -> Vec<CellAddress> {
    let mut dirty = Vec::new();
    for address in changed_cells {
        push_unique(&mut dirty, *address);
    }

    let mut changed = true;
    while changed {
        changed = false;
        for edge in edges {
            if dirty.contains(&edge.precedent) && !dirty.contains(&edge.formula_cell) {
                dirty.push(edge.formula_cell);
                changed = true;
            }
        }
    }
    dirty
}

// r[impl domain-primitives.spreadsheet.admission]
pub fn spreadsheet_admission_composed(checks: SpreadsheetAdmissionChecks) -> bool {
    checks.address_admitted
        && checks.range_admitted
        && checks.formula_admitted
        && checks.dependency_extracted
        && checks.cycle_rejected
        && checks.recalc_order_admitted
        && checks.dirty_propagation_admitted
        && checks.error_envelope_deterministic
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spreadsheet_dependency::SpreadsheetFormulaCell;
    use crate::spreadsheet_formula::SpreadsheetFormula;

    const FIRST_ROW: u16 = 0;
    const INPUT_COLUMN: u16 = 0;
    const FIRST_FORMULA_COLUMN: u16 = 1;
    const SECOND_FORMULA_COLUMN: u16 = 2;
    const UNRELATED_COLUMN: u16 = 3;

    fn cell(column: u16) -> CellAddress {
        CellAddress { row: FIRST_ROW, column }
    }

    fn formula_cell(column: u16, precedent_column: u16) -> SpreadsheetFormulaCell {
        SpreadsheetFormulaCell {
            address: cell(column),
            formula: SpreadsheetFormula::Reference(cell(precedent_column)),
        }
    }

    fn formula_cells() -> Vec<SpreadsheetFormulaCell> {
        vec![
            formula_cell(FIRST_FORMULA_COLUMN, INPUT_COLUMN),
            formula_cell(SECOND_FORMULA_COLUMN, FIRST_FORMULA_COLUMN),
            formula_cell(UNRELATED_COLUMN, UNRELATED_COLUMN),
        ]
    }

    fn edges() -> Vec<SpreadsheetDependencyEdge> {
        vec![
            SpreadsheetDependencyEdge {
                formula_cell: cell(FIRST_FORMULA_COLUMN),
                precedent: cell(INPUT_COLUMN),
            },
            SpreadsheetDependencyEdge {
                formula_cell: cell(SECOND_FORMULA_COLUMN),
                precedent: cell(FIRST_FORMULA_COLUMN),
            },
        ]
    }

    fn all_checks() -> SpreadsheetAdmissionChecks {
        SpreadsheetAdmissionChecks {
            address_admitted: true,
            range_admitted: true,
            formula_admitted: true,
            dependency_extracted: true,
            cycle_rejected: true,
            recalc_order_admitted: true,
            dirty_propagation_admitted: true,
            error_envelope_deterministic: true,
        }
    }

    // r[verify domain-primitives.spreadsheet.recalc]
    #[test]
    fn topological_recalculation_order_is_admitted() {
        let order = [cell(FIRST_FORMULA_COLUMN), cell(SECOND_FORMULA_COLUMN)];
        assert!(spreadsheet_recalc_order_admitted(&formula_cells(), &edges(), &order));
    }

    // r[verify domain-primitives.spreadsheet.recalc]
    #[test]
    fn precedent_after_dependent_order_is_rejected() {
        let order = [cell(SECOND_FORMULA_COLUMN), cell(FIRST_FORMULA_COLUMN)];
        assert!(!spreadsheet_recalc_order_admitted(&formula_cells(), &edges(), &order));
    }

    // r[verify domain-primitives.spreadsheet.dirty-propagation]
    #[test]
    fn dirty_propagation_includes_changed_cells_and_reachable_dependents() {
        let dirty = spreadsheet_dirty_cells(&edges(), &[cell(INPUT_COLUMN)]);
        assert!(dirty.contains(&cell(INPUT_COLUMN)));
        assert!(dirty.contains(&cell(FIRST_FORMULA_COLUMN)));
        assert!(dirty.contains(&cell(SECOND_FORMULA_COLUMN)));
    }

    // r[verify domain-primitives.spreadsheet.dirty-propagation]
    #[test]
    fn dirty_propagation_excludes_unrelated_formula_cells() {
        let dirty = spreadsheet_dirty_cells(&edges(), &[cell(INPUT_COLUMN)]);
        assert!(!dirty.contains(&cell(UNRELATED_COLUMN)));
    }

    // r[verify domain-primitives.spreadsheet.admission]
    #[test]
    fn spreadsheet_admission_requires_every_check() {
        assert!(spreadsheet_admission_composed(all_checks()));
        assert!(!spreadsheet_admission_composed(SpreadsheetAdmissionChecks {
            dependency_extracted: false,
            ..all_checks()
        }));
    }
}
