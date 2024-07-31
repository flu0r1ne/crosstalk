//! This module provides support for CLI Tables
//!
//! CLI Tables are presented in docker-form, except with the addition of color
//! support. Tables are constructed by adding consecutive rows:
//! 
//! ```
//! let mut tab = Table::new();
//! 
//! tab.set_header(vec!["COL_A", "COL_B"]);
//! 
//! tab.add_row(vec!["A1", "B1"])
//! 
//! print!("{}", tab);
//! ```
//! 
//! Rows consist of cells. Cells can by styled using `nu_ansi_term` styles. Additionally,
//! the row contains helper methods for applying style to all cells in a row.
//! 
//! ```
//! use nu_ansi_term::Color;
//! 
//! let mut tab = Table::new();
//! 
//! let header = vec!["COL_A", "COL_B"].into_row().with_style(Color::White.into());
//! 
//! tab.set_header(header);
//! 
//! tab.add_row(vec![
//!     "A1".to_string().into_cell().with_style(Color::Blue.into()),
//!     Cell::new("B1".to_string(), Color::Blue.into())
//! ]);
//! 
//! print!({}, tab);
//! ```

use std::fmt::{self, Write};
use nu_ansi_term::{AnsiGenericString, Style};

pub(crate) struct Cell {
    content: String,
    style: Style,
}

impl Cell {
    pub(crate) fn new(content: String, style: Style) -> Cell {
        Cell { content, style }
    }

    fn is_awk_safe(&self) -> bool {
        !self.content.is_empty() && !self.content.contains(|c: char| c.is_whitespace())
    }

    pub(crate) fn len(&self) -> usize {
        self.content.len()
    }

    pub(crate) fn paint<'a>(&'a self) -> AnsiGenericString<'a, str> {
        self.style.paint(&self.content)
    }

    pub(crate) fn content(&self) -> &str {
        &self.content
    }

    pub(crate) fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl From<String> for Cell {
    fn from(value: String) -> Self {
        Cell { content: value, style: Style::default() }
    }
}

pub(crate) struct Row {
    cells: Vec<Cell>,
}

impl Row {
    pub(crate) fn is_awk_safe(&self) -> bool {
        for cell in &self.cells {
            if !cell.is_awk_safe() {
                return false;
            }
        }

        true
    }

    fn len(&self) -> usize {
        self.cells.len()
    }

    /// Helper to add style to all cells in the row
    pub(crate) fn with_style(mut self, style: Style) -> Self {
        for cell in &mut self.cells {
            cell.style = style;
        }

        self
    }
}

impl From<Vec<String>> for Row {
    fn from(value: Vec<String>) -> Self {
        Row { cells: value.into_iter().map(|v| v.into()).collect() }
    }
}

impl From<Vec<&str>> for Row {
    fn from(value: Vec<&str>) -> Self {
        let value: Vec<String> = value.into_iter().map(|s| s.to_owned()).collect();

        value.into()
    }
}

impl From<Vec<Cell>> for Row {
    fn from(value: Vec<Cell>) -> Self {
        Row { cells: value }
    }
}

pub(crate) struct Table {
    body: Vec<Row>,
    header: Option<Row>,
    num_columns: Option<usize>,
    print_header: bool,
    color: bool,
}

impl Table {
    pub(crate) fn new() -> Table {
        Table {
            body: Vec::new(),
            header: None,
            num_columns: None,
            print_header: true,
            color: true,
        }
    }

    fn expect_num_columns(&mut self, num_columns: usize) {
        if let Some(prev_num_columns) = &self.num_columns {
            if *prev_num_columns == num_columns {
                return;
            }
            panic!(
                "Table has {} columns but a with {} columns was inserted",
                prev_num_columns, num_columns
            );
        } else {
            let _ = self.num_columns.insert(num_columns);
        }
    }

    pub(crate) fn set_color(&mut self, color: bool) {
        self.color = color;
    }

    pub(crate) fn print_header(&mut self, print_header: bool) {
        self.print_header = print_header;
    }

    pub(crate) fn add_row<S: IntoRow>(&mut self, row: S) {
        let row = row.into_row();

        self.expect_num_columns(row.len());

        self.body.push(row);
    }

    pub(crate) fn set_header<S: IntoRow>(&mut self, header: S) {
        let header = header.into_row();

        self.expect_num_columns(header.len());

        if !header.is_awk_safe() {
            panic!("Table header is not awk safe. One of the cells contains a whitespace character or is empty.")
        }

        self.header.replace(header);
    }

    pub(crate) fn header(&self) -> Option<&Row> {
        self.header.as_ref()
    }

    fn iter_rows(&self) -> impl Iterator<Item = &Row> {
        self.header.iter().chain(self.body.iter())
    }

    fn column_widths(&self, include_header: bool) -> Vec<usize> {
        let n_cols = match self.num_columns {
            Some(n_cols) => n_cols,
            None => return Vec::new(),
        };

        let mut widths = vec![0usize; n_cols];

        let mut update_widths = |row: &Row| {
            for (i, cell) in row.cells.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        };

        for row in self.body.iter() {
            update_widths(row)
        }

        if !include_header {
            return widths;
        }

        if let Some(header) = self.header() {
            update_widths(header)
        }

        widths
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let widths = self.column_widths(self.print_header);

        let mut print_row = |row: &Row| -> std::fmt::Result {
            for (i, cell) in row.cells.iter().enumerate() {

                if self.color {
 
                    // Rust formatting does not handle terminal escape sequence,
                    // necessitating manual right-padding
                    f.write_fmt(format_args!("{}", cell.paint() ))?;

                    for _ in 0..(widths[i] - cell.len()) {
                        f.write_char(' ')?;
                    }
                   
                } else {
                    f.write_fmt(format_args!("{:<width$}", cell.content(), width = widths[i]))?;
                }

                if i != row.cells.len() - 1 {
                    f.write_str("  ")?;
                }
            }

            f.write_char('\n')?;

            Ok(())
        };

        match self.print_header {
            true => {
                for row in self.iter_rows() {
                    print_row(row)?;
                }
            }
            false => {
                for row in self.body.iter() {
                    print_row(row)?;
                }
            }
        }

        Ok(())
    }
}

pub(crate) trait IntoTable: Into<Table> + Sized {
    fn into_table(self) -> Table {
        self.into()
    }
}

impl<T> IntoTable for T where T: Into<Table> + Sized {}

pub(crate) trait IntoRow: Into<Row> + Sized {
    fn into_row(self) -> Row {
        self.into()
    }
}

impl<T> IntoRow for T where T: Into<Row> + Sized {}

pub(crate) trait IntoCell: Into<Cell> + Sized {
    fn into_cell(self) -> Cell {
        self.into()
    }
}

impl<T> IntoCell for T where T: Into<Cell> + Sized {}

#[cfg(test)]
mod tests {
    use super::*;
    use nu_ansi_term::Color;

    #[test]
    fn test_construct_table_without_color() {
        let mut tab = Table::new();
        tab.set_color(false); // Disable color

        tab.set_header(vec!["COL_A", "COL_B"]);
        tab.add_row(vec!["A1", "B1"]);
        tab.add_row(vec!["A2", "B2"]);

        let expected = "COL_A  COL_B\nA1     B1   \nA2     B2   \n";
        assert_eq!(format!("{}", tab), expected);
    }

    #[test]
    fn test_construct_table_with_color() {
        let mut tab = Table::new();
        tab.set_color(true); // Enable color

        let header = vec!["COL_A", "COL_B"].into_row().with_style(Color::White.normal());
        tab.set_header(header);

        tab.add_row(vec![
            "A1".to_string().into_cell().with_style(Color::Blue.normal()),
            Cell::new("B1".to_string(), Color::Blue.normal())
        ]);

        let styled_expected = Color::White.paint("COL_A").to_string() +
                              "  " + &Color::White.paint("COL_B").to_string() + "\n" +
                              &Color::Blue.paint("A1").to_string() + "     " +
                              &Color::Blue.paint("B1").to_string() + "   \n";
        assert_eq!(format!("{}", tab), styled_expected);
    }

    #[test]
    fn test_set_color() {
        let mut tab = Table::new();
        tab.set_color(false); // Disable color

        let header = vec!["COL_A", "COL_B"].into_row().with_style(Color::White.normal());
        tab.set_header(header);

        tab.add_row(vec![
            "A1".to_string().into_cell().with_style(Color::Blue.normal()),
            Cell::new("B1".to_string(), Color::Blue.normal())
        ]);

        let expected = "COL_A  COL_B\nA1     B1   \n";
        assert_eq!(format!("{}", tab), expected);
    }

    #[test]
    fn test_print_header() {
        let mut tab = Table::new();
        tab.print_header(false); // Disable header

        tab.set_header(vec!["COL_A", "COL_B"]);
        tab.add_row(vec!["A1", "B1"]);
        tab.add_row(vec!["A2", "B2"]);

        let expected = "A1  B1\nA2  B2\n";
        assert_eq!(format!("{}", tab), expected);
    }

    #[test]
    #[should_panic(expected = "Table header is not awk safe. One of the cells contains a whitespace character or is empty.")]
    fn test_non_awk_safe_header() {
        let mut tab = Table::new();

        tab.set_header(vec!["COL A", "COL_B"]);
    }
}