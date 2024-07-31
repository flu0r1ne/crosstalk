use std::fmt::{self, Write};

pub(crate) struct Row {
    cells: Vec<String>,
}

impl Row {
    fn is_awk_safe(&self) -> bool {
        for cell in &self.cells {
            if cell.contains(|c: char| c.is_whitespace()) {
                return false;
            }
        }

        true
    }

    fn columns(&self) -> usize {
        self.cells.len()
    }
}

pub(crate) trait IntoRow: Into<Row> + Sized {
    fn into_row(self) -> Row {
        self.into()
    }
}

impl<T> IntoRow for T where T: Into<Row> + Sized {}

impl From<Vec<String>> for Row {
    fn from(value: Vec<String>) -> Self {
        Row { cells: value }
    }
}

impl From<Vec<&str>> for Row {
    fn from(value: Vec<&str>) -> Self {
        let value: Vec<String> = value.into_iter().map(|s| s.to_owned()).collect();

        value.into()
    }
}

pub(crate) struct Table {
    body: Vec<Row>,
    header: Option<Row>,
    num_columns: Option<usize>,
    print_header: bool,
}

impl Table {
    pub(crate) fn new() -> Table {
        Table {
            body: Vec::new(),
            header: None,
            num_columns: None,
            print_header: true,
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

    pub(crate) fn print_header(&mut self, print_header: bool) {
        self.print_header = print_header;
    }

    pub(crate) fn add_row<S: IntoRow>(&mut self, row: S) {
        let row = row.into_row();

        self.expect_num_columns(row.columns());

        self.body.push(row);
    }

    pub(crate) fn set_header<S: IntoRow>(&mut self, header: S) {
        let header = header.into_row();

        self.expect_num_columns(header.columns());

        if !header.is_awk_safe() {
            panic!("Table header is not awk safe, contains whitespace")
        }

        self.header.replace(header);
    }

    fn iter_rows(&self) -> impl Iterator<Item = &Row> {
        self.header.iter().chain(self.body.iter())
    }

    fn column_widths(&self) -> Vec<usize> {
        let n_cols = match self.num_columns {
            Some(n_cols) => n_cols,
            None => return Vec::new(),
        };

        let mut widths = vec![0usize; n_cols];

        for row in self.iter_rows() {
            for (i, cell) in row.cells.iter().enumerate() {
                widths[i] = widths[i].max(cell.len());
            }
        }

        widths
    }
}

impl fmt::Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let widths = self.column_widths();

        let mut print_row = |row: &Row| -> std::fmt::Result {
            for (i, cell) in row.cells.iter().enumerate() {
                f.write_fmt(format_args!("{:<width$}", cell, width = widths[i]))?;

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
