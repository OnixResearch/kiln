use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style},
    widgets::{Block, Cell, Row, Table, TableState},
};

/// A scrollable data table widget for ratatui with auto column sizing
#[derive(Debug, Clone)]
pub struct DataTable {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    column_widths: Vec<u16>,
    selected_row: usize,
    scroll_col: usize,
    max_cell_width: u16,
    min_cell_width: u16,
}

/// Styling configuration for the DataTable
#[derive(Debug, Clone)]
pub struct DataTableStyle {
    pub header_style: Style,
    pub selected_style: Style,
    pub normal_style: Style,
    pub truncation_suffix: String,
    pub column_spacing: u16,
}

/// Information about the table for rendering info bars
#[derive(Debug, Clone)]
pub struct DataTableInfo {
    pub row_count: usize,
    pub column_count: usize,
    pub is_truncated: bool,
    pub extra: Vec<(String, String)>,
}

impl Default for DataTableStyle {
    fn default() -> Self {
        Self {
            header_style: Style::default().bold().yellow(),
            selected_style: Style::default().bg(Color::DarkGray).bold(),
            normal_style: Style::default(),
            truncation_suffix: "...".to_string(),
            column_spacing: 1,
        }
    }
}

impl DataTable {
    /// Create a new DataTable with the given columns and rows
    pub fn new(columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        let max_cell_width = 40;
        let min_cell_width = 5;
        let column_widths =
            calculate_column_widths(&columns, &rows, min_cell_width, max_cell_width);

        Self {
            columns,
            rows,
            column_widths,
            selected_row: 0,
            scroll_col: 0,
            max_cell_width,
            min_cell_width,
        }
    }

    /// Set the maximum cell width (builder pattern)
    pub fn with_max_cell_width(mut self, width: u16) -> Self {
        self.max_cell_width = width;
        self.recalculate_widths();
        self
    }

    /// Set the minimum cell width (builder pattern)
    pub fn with_min_cell_width(mut self, width: u16) -> Self {
        self.min_cell_width = width;
        self.recalculate_widths();
        self
    }

    /// Replace the table data and recalculate column widths
    pub fn set_data(&mut self, columns: Vec<String>, rows: Vec<Vec<String>>) {
        self.columns = columns;
        self.rows = rows;
        self.selected_row = 0;
        self.scroll_col = 0;
        self.recalculate_widths();
    }

    /// Move selection to the next row
    pub fn select_next(&mut self) {
        if !self.rows.is_empty() {
            self.selected_row = (self.selected_row + 1).min(self.rows.len().saturating_sub(1));
        }
    }

    /// Move selection to the previous row
    pub fn select_prev(&mut self) {
        if self.selected_row > 0 {
            self.selected_row = self.selected_row.saturating_sub(1);
        }
    }

    /// Move selection to the first row
    pub fn select_first(&mut self) {
        self.selected_row = 0;
    }

    /// Move selection to the last row
    pub fn select_last(&mut self) {
        if !self.rows.is_empty() {
            self.selected_row = self.rows.len().saturating_sub(1);
        }
    }

    /// Scroll right to show more columns
    pub fn scroll_right(&mut self) {
        if self.scroll_col < self.columns.len().saturating_sub(1) {
            self.scroll_col = self.scroll_col.saturating_add(1);
        }
    }

    /// Scroll left to show previous columns
    pub fn scroll_left(&mut self) {
        if self.scroll_col > 0 {
            self.scroll_col = self.scroll_col.saturating_sub(1);
        }
    }

    /// Get the currently selected row's data
    pub fn selected_row(&self) -> Option<&[String]> {
        self.rows.get(self.selected_row).map(|row| row.as_slice())
    }

    /// Get the index of the currently selected row
    pub fn selected_index(&self) -> usize {
        self.selected_row
    }

    /// Get the number of data rows (excluding header)
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get the number of columns
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Clear all data from the table
    pub fn clear(&mut self) {
        self.columns.clear();
        self.rows.clear();
        self.column_widths.clear();
        self.selected_row = 0;
        self.scroll_col = 0;
    }

    /// Get table info for rendering info bars
    pub fn info(&self) -> DataTableInfo {
        // Check if any cells were truncated during width calculation
        let is_truncated = self.rows.iter().any(|row| {
            row.iter().enumerate().any(|(i, cell)| {
                if let Some(&width) = self.column_widths.get(i) {
                    cell.len() as u16 > width
                } else {
                    false
                }
            })
        });

        DataTableInfo {
            row_count: self.row_count(),
            column_count: self.column_count(),
            is_truncated,
            extra: vec![],
        }
    }

    /// Render the data table
    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        block: Option<Block>,
        style: &DataTableStyle,
    ) {
        let render_area = if let Some(block) = block.clone() {
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        } else {
            area
        };

        // If no data, render empty table
        if self.columns.is_empty() {
            let table = Table::new(vec![] as Vec<Row>, vec![] as Vec<Constraint>);
            frame.render_widget(table, render_area);
            return;
        }

        // Determine visible columns based on scroll offset and available width
        let visible_columns = self.get_visible_columns(render_area.width);

        if visible_columns.is_empty() {
            // No visible columns
            let table = Table::new(vec![] as Vec<Row>, vec![] as Vec<Constraint>);
            frame.render_widget(table, render_area);
            return;
        }

        // Create constraints for visible columns
        let constraints: Vec<Constraint> = visible_columns
            .iter()
            .map(|(_, width)| Constraint::Length(*width + style.column_spacing))
            .collect();

        // Create header row
        let header_cells: Vec<Cell> = visible_columns
            .iter()
            .map(|(col_name, width)| {
                let display_name = truncate_cell(col_name, *width, &style.truncation_suffix);
                Cell::from(display_name).style(style.header_style)
            })
            .collect();
        let header_row = Row::new(header_cells);

        // Create data rows
        let data_rows: Vec<Row> = self
            .rows
            .iter()
            .enumerate()
            .map(|(row_idx, row_data)| {
                let cells: Vec<Cell> = visible_columns
                    .iter()
                    .enumerate()
                    .map(|(visible_idx, (_, width))| {
                        let col_idx = self.scroll_col + visible_idx;
                        let cell_content = row_data.get(col_idx).map(|s| s.as_str()).unwrap_or("");
                        let display_content =
                            truncate_cell(cell_content, *width, &style.truncation_suffix);

                        let cell_style = if row_idx == self.selected_row {
                            style.selected_style
                        } else {
                            style.normal_style
                        };

                        Cell::from(display_content).style(cell_style)
                    })
                    .collect();
                Row::new(cells)
            })
            .collect();

        // Build table
        let table = Table::new(data_rows, constraints).header(header_row);

        // Create and configure table state for selection
        let mut table_state = TableState::default();
        if !self.rows.is_empty() {
            table_state.select(Some(self.selected_row));
        }

        frame.render_stateful_widget(table, render_area, &mut table_state);
    }

    /// Recalculate column widths based on current min/max settings
    fn recalculate_widths(&mut self) {
        self.column_widths = calculate_column_widths(
            &self.columns,
            &self.rows,
            self.min_cell_width,
            self.max_cell_width,
        );
    }

    /// Get visible columns that fit within the given width
    fn get_visible_columns(&self, available_width: u16) -> Vec<(String, u16)> {
        let mut visible = Vec::new();
        let mut used_width = 0;

        for i in self.scroll_col..self.columns.len() {
            if let (Some(col_name), Some(&col_width)) =
                (self.columns.get(i), self.column_widths.get(i))
            {
                let needed_width = col_width + if visible.is_empty() { 0 } else { 1 }; // column spacing

                if used_width + needed_width <= available_width {
                    visible.push((col_name.clone(), col_width));
                    used_width += needed_width;
                } else {
                    break;
                }
            }
        }

        visible
    }
}

/// Calculate optimal column widths based on content and constraints
fn calculate_column_widths(
    columns: &[String],
    rows: &[Vec<String>],
    min_width: u16,
    max_width: u16,
) -> Vec<u16> {
    let mut widths: Vec<u16> = columns
        .iter()
        .map(|col| (col.len() as u16).clamp(min_width, max_width))
        .collect();

    for row in rows {
        for (col_idx, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(col_idx) {
                let cell_width = (cell.len() as u16).clamp(min_width, max_width);
                *width = (*width).max(cell_width);
            }
        }
    }

    widths
}

/// Truncate cell content to fit within the specified width
fn truncate_cell(content: &str, max_width: u16, suffix: &str) -> String {
    if content.len() <= max_width as usize {
        content.to_string()
    } else if max_width <= suffix.len() as u16 {
        suffix.chars().take(max_width as usize).collect()
    } else {
        let content_width = max_width as usize - suffix.len();
        format!("{}{}", &content[..content_width], suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_table_creation() {
        let columns = vec!["Name".to_string(), "Age".to_string()];
        let rows = vec![
            vec!["Alice".to_string(), "25".to_string()],
            vec!["Bob".to_string(), "30".to_string()],
        ];

        let table = DataTable::new(columns.clone(), rows.clone());

        assert_eq!(table.columns, columns);
        assert_eq!(table.rows, rows);
        assert_eq!(table.selected_row, 0);
        assert_eq!(table.scroll_col, 0);
        assert_eq!(table.row_count(), 2);
        assert_eq!(table.column_count(), 2);
    }

    #[test]
    fn test_navigation() {
        let mut table = DataTable::new(
            vec!["Col1".to_string()],
            vec![
                vec!["Row1".to_string()],
                vec!["Row2".to_string()],
                vec!["Row3".to_string()],
            ],
        );

        assert_eq!(table.selected_index(), 0);

        table.select_next();
        assert_eq!(table.selected_index(), 1);

        table.select_last();
        assert_eq!(table.selected_index(), 2);

        table.select_prev();
        assert_eq!(table.selected_index(), 1);

        table.select_first();
        assert_eq!(table.selected_index(), 0);
    }

    #[test]
    fn test_scrolling() {
        let mut table = DataTable::new(
            vec!["Col1".to_string(), "Col2".to_string(), "Col3".to_string()],
            vec![vec!["A".to_string(), "B".to_string(), "C".to_string()]],
        );

        assert_eq!(table.scroll_col, 0);

        table.scroll_right();
        assert_eq!(table.scroll_col, 1);

        table.scroll_right();
        assert_eq!(table.scroll_col, 2);

        // Should not scroll past last column
        table.scroll_right();
        assert_eq!(table.scroll_col, 2);

        table.scroll_left();
        assert_eq!(table.scroll_col, 1);
    }

    #[test]
    fn test_column_width_calculation() {
        let columns = vec!["Short".to_string(), "Very Long Header".to_string()];
        let rows = vec![
            vec!["A".to_string(), "Small".to_string()],
            vec!["Very Long Content".to_string(), "B".to_string()],
        ];

        let widths = calculate_column_widths(&columns, &rows, 5, 40);

        // First column: max("Short".len(), "Very Long Content".len()) = 17
        // Second column: max("Very Long Header".len(), "Small".len()) = 16
        assert_eq!(widths[0], 17);
        assert_eq!(widths[1], 16);
    }

    #[test]
    fn test_truncation() {
        let result = truncate_cell("This is a very long string", 10, "...");
        assert_eq!(result, "This is..."); // 10 - 3 = 7 chars of content

        let result = truncate_cell("Short", 10, "...");
        assert_eq!(result, "Short");

        // Test edge case where suffix is longer than max width
        let result = truncate_cell("Test", 2, "...");
        assert_eq!(result, "..");
    }

    #[test]
    fn test_clear() {
        let mut table = DataTable::new(vec!["Col1".to_string()], vec![vec!["Row1".to_string()]]);

        assert!(!table.is_empty());
        table.clear();
        assert!(table.is_empty());
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
    }

    #[test]
    fn test_selected_row() {
        let mut table = DataTable::new(
            vec!["Name".to_string()],
            vec![vec!["Alice".to_string()], vec!["Bob".to_string()]],
        );

        assert_eq!(
            table.selected_row(),
            Some(vec!["Alice".to_string()].as_slice())
        );

        table.select_next();
        assert_eq!(
            table.selected_row(),
            Some(vec!["Bob".to_string()].as_slice())
        );
    }

    #[test]
    fn empty_table_operations() {
        let mut table = DataTable::new(vec![], vec![]);

        assert!(table.is_empty());
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        assert_eq!(table.selected_row(), None);
        assert_eq!(table.selected_index(), 0);

        // Navigation on empty table should not panic.
        table.select_next();
        table.select_prev();
        table.select_first();
        table.select_last();
        table.scroll_left();
        table.scroll_right();
        assert_eq!(table.selected_index(), 0);
    }

    #[test]
    fn navigation_clamps_at_boundaries() {
        let mut table = DataTable::new(
            vec!["X".to_string()],
            vec![vec!["a".to_string()], vec!["b".to_string()]],
        );

        // prev at 0 stays at 0
        table.select_prev();
        assert_eq!(table.selected_index(), 0);

        // next past last stays at last
        table.select_last();
        assert_eq!(table.selected_index(), 1);
        table.select_next();
        assert_eq!(table.selected_index(), 1);
    }

    #[test]
    fn scroll_clamps_at_boundaries() {
        let mut table = DataTable::new(
            vec!["A".to_string(), "B".to_string()],
            vec![vec!["1".to_string(), "2".to_string()]],
        );

        // left at 0 stays at 0
        table.scroll_left();
        assert_eq!(table.scroll_col, 0);

        // right stops at last column index
        table.scroll_right();
        assert_eq!(table.scroll_col, 1);
        table.scroll_right();
        assert_eq!(table.scroll_col, 1);
    }

    #[test]
    fn with_max_cell_width_clamps_columns() {
        let table = DataTable::new(
            vec!["Header".to_string()],
            vec![vec![
                "A very long cell value that exceeds the cap".to_string(),
            ]],
        )
        .with_max_cell_width(10);

        assert!(table.column_widths.iter().all(|&w| w <= 10));
    }

    #[test]
    fn with_min_cell_width_enforces_floor() {
        let table = DataTable::new(vec!["H".to_string()], vec![vec!["X".to_string()]])
            .with_min_cell_width(12);

        assert!(table.column_widths.iter().all(|&w| w >= 12));
    }

    #[test]
    fn set_data_resets_state() {
        let mut table = DataTable::new(
            vec!["Old".to_string()],
            vec![
                vec!["r1".to_string()],
                vec!["r2".to_string()],
                vec!["r3".to_string()],
            ],
        );
        table.select_last();
        table.scroll_right();

        table.set_data(
            vec!["New".to_string(), "Cols".to_string()],
            vec![vec!["a".to_string(), "b".to_string()]],
        );

        assert_eq!(table.selected_index(), 0);
        assert_eq!(table.scroll_col, 0);
        assert_eq!(table.column_count(), 2);
        assert_eq!(table.row_count(), 1);
    }

    #[test]
    fn info_reports_truncation() {
        let table = DataTable::new(
            vec!["Name".to_string()],
            vec![vec![
                "A string longer than the forty character maximum width".to_string(),
            ]],
        );

        let info = table.info();
        assert!(info.is_truncated);
        assert_eq!(info.row_count, 1);
        assert_eq!(info.column_count, 1);
    }

    #[test]
    fn info_no_truncation_for_short_cells() {
        let table = DataTable::new(vec!["Name".to_string()], vec![vec!["Short".to_string()]]);

        let info = table.info();
        assert!(!info.is_truncated);
    }

    #[test]
    fn column_widths_clamped_to_min_max() {
        let widths = calculate_column_widths(&["X".to_string()], &[vec!["Y".to_string()]], 8, 20);
        // Both "X" and "Y" are 1 char, but min is 8.
        assert_eq!(widths[0], 8);

        let widths = calculate_column_widths(
            &["A".to_string()],
            &[vec![
                "This string is way beyond the twenty char max".to_string(),
            ]],
            5,
            20,
        );
        assert_eq!(widths[0], 20);
    }

    #[test]
    fn rows_with_fewer_columns_than_header() {
        let table = DataTable::new(
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
            vec![vec!["only_one".to_string()]],
        );

        // Should have widths for all 3 header columns.
        assert_eq!(table.column_widths.len(), 3);
        // Missing cells don't crash selected_row.
        assert_eq!(table.selected_row().unwrap().len(), 1);
    }

    #[test]
    fn visible_columns_respects_scroll_and_width() {
        let table = DataTable::new(
            vec![
                "AAAAAAAAAA".to_string(),
                "BBBBBBBBBB".to_string(),
                "CCCCCCCCCC".to_string(),
            ],
            vec![vec![
                "1234567890".to_string(),
                "1234567890".to_string(),
                "1234567890".to_string(),
            ]],
        );

        // With enough width, all columns visible from scroll 0.
        let visible = table.get_visible_columns(200);
        assert_eq!(visible.len(), 3);

        // With tight width, fewer columns visible.
        let visible = table.get_visible_columns(15);
        assert_eq!(visible.len(), 1);
    }
}
