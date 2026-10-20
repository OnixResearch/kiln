//! Spreadsheet dependency extraction and cycle admission over finite formula cells.

use crate::spreadsheet_address::CellAddress;
use crate::spreadsheet_address::SheetBounds;
use crate::spreadsheet_address::spreadsheet_range_expand;
use crate::spreadsheet_formula::SpreadsheetFormula;
use crate::spreadsheet_formula::SpreadsheetFormulaError;
use crate::spreadsheet_formula::spreadsheet_formula_admitted;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpreadsheetFormulaCell {
    pub address: CellAddress,
    pub formula: SpreadsheetFormula,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpreadsheetDependencyEdge {
    pub formula_cell: CellAddress,
    pub precedent: CellAddress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VisitMark {
    Unseen,
    Visiting,
    Done,
}

fn push_unique(cells: &mut Vec<CellAddress>, address: CellAddress) {
    if !cells.contains(&address) {
        cells.push(address);
    }
}

fn append_unique(target: &mut Vec<CellAddress>, source: Vec<CellAddress>) {
    for address in source {
        push_unique(target, address);
    }
}

// r[impl domain-primitives.spreadsheet.dependency]
pub fn spreadsheet_formula_dependencies(
    bounds: SheetBounds,
    formula: &SpreadsheetFormula,
) -> Result<Vec<CellAddress>, SpreadsheetFormulaError> {
    if !spreadsheet_formula_admitted(bounds, formula) {
        return Err(SpreadsheetFormulaError::BadReference);
    }
    let dependencies = match formula {
        SpreadsheetFormula::Constant(_) => Vec::new(),
        SpreadsheetFormula::Reference(address) => vec![*address],
        SpreadsheetFormula::RangeSum(range)
        | SpreadsheetFormula::RangeCount(range)
        | SpreadsheetFormula::RangeMin(range)
        | SpreadsheetFormula::RangeMax(range) => spreadsheet_range_expand(*range),
        SpreadsheetFormula::Add(left, right) | SpreadsheetFormula::Divide(left, right) => {
            let mut combined = spreadsheet_formula_dependencies(bounds, left)?;
            append_unique(&mut combined, spreadsheet_formula_dependencies(bounds, right)?);
            combined
        }
    };
    Ok(dependencies)
}

// r[impl domain-primitives.spreadsheet.dependency]
pub fn spreadsheet_formula_set_dependencies(
    bounds: SheetBounds,
    cells: &[SpreadsheetFormulaCell],
) -> Result<Vec<SpreadsheetDependencyEdge>, SpreadsheetFormulaError> {
    let mut edges = Vec::new();
    for cell in cells {
        for precedent in spreadsheet_formula_dependencies(bounds, &cell.formula)? {
            edges.push(SpreadsheetDependencyEdge {
                formula_cell: cell.address,
                precedent,
            });
        }
    }
    Ok(edges)
}

fn formula_cell_index(cells: &[SpreadsheetFormulaCell], address: CellAddress) -> Option<usize> {
    cells.iter().position(|cell| cell.address == address)
}

fn visit_cycle(
    index: usize,
    marks: &mut [VisitMark],
    cells: &[SpreadsheetFormulaCell],
    edges: &[SpreadsheetDependencyEdge],
) -> bool {
    match marks[index] {
        VisitMark::Visiting => return true,
        VisitMark::Done => return false,
        VisitMark::Unseen => {}
    }
    marks[index] = VisitMark::Visiting;
    let current = cells[index].address;
    for edge in edges.iter().filter(|edge| edge.formula_cell == current) {
        if let Some(next_index) = formula_cell_index(cells, edge.precedent) {
            if visit_cycle(next_index, marks, cells, edges) {
                return true;
            }
        }
    }
    marks[index] = VisitMark::Done;
    false
}

// r[impl domain-primitives.spreadsheet.cycle]
pub fn spreadsheet_formula_set_has_cycle(bounds: SheetBounds, cells: &[SpreadsheetFormulaCell]) -> bool {
    let Ok(edges) = spreadsheet_formula_set_dependencies(bounds, cells) else {
        return true;
    };
    let mut marks = vec![VisitMark::Unseen; cells.len()];
    for index in 0..cells.len() {
        if visit_cycle(index, &mut marks, cells, &edges) {
            return true;
        }
    }
    false
}

// r[impl domain-primitives.spreadsheet.cycle]
pub fn spreadsheet_formula_set_admitted(bounds: SheetBounds, cells: &[SpreadsheetFormulaCell]) -> bool {
    cells.iter().all(|cell| spreadsheet_formula_admitted(bounds, &cell.formula))
        && !spreadsheet_formula_set_has_cycle(bounds, cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spreadsheet_address::SpreadsheetRange;
    use crate::spreadsheet_formula::SpreadsheetValue;

    const ROWS: u16 = 3;
    const COLUMNS: u16 = 3;
    const FIRST_ROW: u16 = 0;
    const FIRST_COLUMN: u16 = 0;
    const SECOND_COLUMN: u16 = 1;
    const THIRD_COLUMN: u16 = 2;
    const CONSTANT_VALUE: i64 = 5;
    const RANGE_DEP_COUNT: usize = 2;

    fn bounds() -> SheetBounds {
        SheetBounds {
            row_count: ROWS,
            column_count: COLUMNS,
        }
    }

    fn cell(column: u16) -> CellAddress {
        CellAddress { row: FIRST_ROW, column }
    }

    fn formula_cell(column: u16, formula: SpreadsheetFormula) -> SpreadsheetFormulaCell {
        SpreadsheetFormulaCell {
            address: cell(column),
            formula,
        }
    }

    // r[verify domain-primitives.spreadsheet.dependency]
    #[test]
    fn range_dependency_expands_to_contained_cells_and_constants_have_none() {
        let range_formula = SpreadsheetFormula::RangeSum(SpreadsheetRange {
            start: cell(FIRST_COLUMN),
            end: cell(SECOND_COLUMN),
        });
        let deps = spreadsheet_formula_dependencies(bounds(), &range_formula).expect("range dependencies");
        assert_eq!(deps.len(), RANGE_DEP_COUNT);
        assert!(deps.contains(&cell(FIRST_COLUMN)));
        assert!(deps.contains(&cell(SECOND_COLUMN)));

        let constant = SpreadsheetFormula::Constant(SpreadsheetValue::Number(CONSTANT_VALUE));
        assert!(spreadsheet_formula_dependencies(bounds(), &constant).expect("constant dependencies").is_empty());
    }

    // r[verify domain-primitives.spreadsheet.dependency]
    #[test]
    fn nested_formula_dependencies_are_exact_and_deduplicated() {
        let formula = SpreadsheetFormula::Add(
            Box::new(SpreadsheetFormula::Reference(cell(FIRST_COLUMN))),
            Box::new(SpreadsheetFormula::Reference(cell(FIRST_COLUMN))),
        );
        let deps = spreadsheet_formula_dependencies(bounds(), &formula).expect("nested dependencies");
        assert_eq!(deps, vec![cell(FIRST_COLUMN)]);
    }

    // r[verify domain-primitives.spreadsheet.cycle]
    #[test]
    fn direct_and_indirect_cycles_are_rejected_before_evaluation() {
        let direct = [formula_cell(
            FIRST_COLUMN,
            SpreadsheetFormula::Reference(cell(FIRST_COLUMN)),
        )];
        assert!(spreadsheet_formula_set_has_cycle(bounds(), &direct));
        assert!(!spreadsheet_formula_set_admitted(bounds(), &direct));

        let indirect = [
            formula_cell(FIRST_COLUMN, SpreadsheetFormula::Reference(cell(SECOND_COLUMN))),
            formula_cell(SECOND_COLUMN, SpreadsheetFormula::Reference(cell(THIRD_COLUMN))),
            formula_cell(THIRD_COLUMN, SpreadsheetFormula::Reference(cell(FIRST_COLUMN))),
        ];
        assert!(spreadsheet_formula_set_has_cycle(bounds(), &indirect));
    }

    // r[verify domain-primitives.spreadsheet.cycle]
    #[test]
    fn acyclic_formula_set_is_admitted() {
        let acyclic = [
            formula_cell(SECOND_COLUMN, SpreadsheetFormula::Reference(cell(FIRST_COLUMN))),
            formula_cell(THIRD_COLUMN, SpreadsheetFormula::Reference(cell(SECOND_COLUMN))),
        ];
        assert!(!spreadsheet_formula_set_has_cycle(bounds(), &acyclic));
        assert!(spreadsheet_formula_set_admitted(bounds(), &acyclic));
    }
}
