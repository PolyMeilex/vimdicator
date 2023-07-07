use std::collections::{hash_map, HashMap};

use log::error;

use super::event::GridLineCell;

#[derive(Debug, Default)]
pub struct ExtLineGridMap {
    map: HashMap<u64, ExtLineGrid>,
}

impl ExtLineGridMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_default(&self) -> Option<&ExtLineGrid> {
        self.get(&1)
    }

    pub fn get(&self, grid: &u64) -> Option<&ExtLineGrid> {
        self.map.get(grid)
    }

    pub fn grid_resize(&mut self, grid: &u64, columns: usize, rows: usize) {
        match self.map.entry(*grid) {
            hash_map::Entry::Occupied(mut grid) => {
                grid.get_mut().resize(columns, rows);
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(ExtLineGrid::new(columns, rows));
            }
        }
    }

    pub fn grid_clear(&mut self, grid: &u64) {
        if let Some(grid) = self.map.get_mut(grid) {
            grid.clear();
        } else {
            error!("Grid '{grid}' not found");
            debug_assert!(false, "Grid '{grid}' not found");
        }
    }

    pub fn grid_destroy(&mut self, grid: &u64) {
        self.map.remove(grid);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn grid_scroll(
        &mut self,
        grid: &u64,
        top: u64,
        bottom: u64,
        left: u64,
        right: u64,
        rows: i64,
        columns: i64,
    ) {
        if let Some(grid) = self.map.get_mut(grid) {
            grid.scroll(top, bottom, left, right, rows, columns);
        } else {
            error!("Grid '{grid}' not found");
            debug_assert!(false, "Grid '{grid}' not found");
        }
    }

    pub fn grid_line(
        &mut self,
        grid: &u64,
        row: usize,
        column_start: usize,
        cells: &[GridLineCell],
    ) {
        if let Some(grid) = self.map.get_mut(grid) {
            grid.update_line(row, column_start, cells);
        } else {
            error!("Grid '{grid}' not found");
            debug_assert!(false, "Grid '{grid}' not found");
        }
    }

    pub fn grid_cursor_goto(&mut self, grid: &u64, row: usize, column: usize) {
        if let Some(grid) = self.map.get_mut(grid) {
            grid.update_cursor(row, column);
        } else {
            error!("Grid '{grid}' not found");
            debug_assert!(false, "Grid '{grid}' not found");
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtLineGrid {
    columns: usize,
    rows: usize,

    cursor_position: CursorPosition,
    buffer: Vec<Line>,
}

#[derive(Debug, Clone)]
pub struct Line {
    columns: Vec<char>,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            columns: vec![' '; len],
        }
    }

    pub fn columns(&self) -> &[char] {
        &self.columns
    }
}

#[derive(Debug, Clone)]
pub struct CursorPosition {
    pub column: usize,
    pub row: usize,
}

impl ExtLineGrid {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            cursor_position: CursorPosition { column: 0, row: 0 },
            buffer: vec![Line::new(columns); rows],
        }
    }

    pub fn columns(&self) -> usize {
        self.columns
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cursor_position(&self) -> &CursorPosition {
        &self.cursor_position
    }

    pub fn buffer(&self) -> &[Line] {
        &self.buffer
    }

    fn clear(&mut self) {
        self.buffer
            .iter_mut()
            .for_each(|line| line.columns.fill(' '));
    }

    fn scroll(
        &mut self,
        _top: u64,
        _bottom: u64,
        _left: u64,
        _right: u64,
        rows: i64,
        _columns: i64,
    ) {
        match rows.cmp(&0) {
            std::cmp::Ordering::Greater => {
                let rows = rows as usize;

                self.buffer.drain(..rows);
                for _ in 0..rows {
                    self.buffer.push(Line::new(self.columns));
                }
            }
            std::cmp::Ordering::Less => {
                let rows = -rows as usize;

                self.buffer.drain(self.buffer.len() - rows..);
                for _ in 0..rows {
                    self.buffer.insert(0, Line::new(self.columns));
                }
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    fn resize(&mut self, columns: usize, rows: usize) {
        match (self.columns != columns, self.rows != rows) {
            // Columns changed
            (true, false) => {
                self.columns = columns;
                self.buffer.iter_mut().for_each(|line| {
                    line.columns.resize(self.columns, ' ');
                });
            }
            // Rows changed
            (false, true) => {
                self.rows = rows;
                self.buffer.resize(self.rows, Line::new(self.columns));
            }
            // Both changed
            (true, true) => {
                // Benchmark if full realoc is faster or not
                // self.buffer = vec![Line::new(columns); rows];

                self.buffer.resize(rows, Line::new(columns));
                self.buffer.iter_mut().for_each(|line| {
                    line.columns.resize(columns, ' ');
                });

                self.columns = columns;
                self.rows = rows;
            }
            (false, false) => {}
        }

        if self.columns == columns && self.rows != rows {
            self.rows = rows;
            self.buffer.resize(self.rows, Line::new(self.columns));
        } else {
            self.buffer = vec![Line::new(self.columns); self.rows];
        }
    }

    fn update_line(&mut self, row: usize, column_start: usize, cells: &[GridLineCell]) {
        let line = &mut self.buffer[row];

        let mut column = column_start;

        for cell in cells {
            let text = &cell.text;
            let repeat = cell.repeat.unwrap_or(1);

            for _ in 0..repeat {
                line.columns[column] = text.chars().next().unwrap_or(' ');
                column += 1;
            }
        }
    }

    fn update_cursor(&mut self, row: usize, column: usize) {
        self.cursor_position.row = row;
        self.cursor_position.column = column;
    }
}
