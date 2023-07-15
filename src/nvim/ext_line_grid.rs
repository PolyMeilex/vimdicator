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

    pub fn get_default_mut(&mut self) -> Option<&mut ExtLineGrid> {
        self.map.get_mut(&1)
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
    pub style: HashMap<u64, super::Style>,
}

#[derive(Debug, Clone)]
pub struct Line {
    columns: Vec<GridLineCell>,
}

impl Line {
    fn new(len: usize) -> Self {
        Self {
            columns: vec![GridLineCell::empty(); len],
        }
    }

    pub fn columns(&self) -> &[GridLineCell] {
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
            style: Default::default(),
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
            .for_each(|line| line.columns.fill(GridLineCell::empty()));
    }

    fn scroll(&mut self, top: u64, bottom: u64, left: u64, right: u64, rows: i64, _columns: i64) {
        let top = top as usize;
        let bottom = bottom as usize;
        let left = left as usize;
        let right = right as usize;

        match rows.cmp(&0) {
            std::cmp::Ordering::Greater => {
                let rows = rows as usize;

                for n in top..bottom - rows {
                    let (to, from) = self.buffer.split_at_mut(n + rows);

                    let from = &mut from[0];
                    let to = &mut to[n];

                    let from = &mut from.columns[left..right];
                    let to = &mut to.columns[left..right];

                    to.swap_with_slice(from);
                }
            }
            std::cmp::Ordering::Less => {
                let rows = -rows as usize;

                for n in ((top + rows)..bottom).rev() {
                    let (from, to) = self.buffer.split_at_mut(n);

                    let from = &mut from[n - rows];
                    let to = &mut to[0];

                    let from = &mut from.columns[left..right];
                    let to = &mut to.columns[left..right];

                    from.swap_with_slice(to);
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
                    line.columns.resize(self.columns, GridLineCell::empty());
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
                    line.columns.resize(columns, GridLineCell::empty());
                });

                self.columns = columns;
                self.rows = rows;
            }
            (false, false) => {}
        }
    }

    fn update_line(&mut self, row: usize, column_start: usize, cells: &[GridLineCell]) {
        let line = &mut self.buffer[row];

        let mut column = column_start;

        for cell in cells {
            let repeat = cell.repeat.unwrap_or(1);

            for _ in 0..repeat {
                let mut cell = cell.clone();
                cell.repeat = None;

                line.columns[column] = cell;
                column += 1;
            }
        }
    }

    fn update_cursor(&mut self, row: usize, column: usize) {
        self.cursor_position.row = row;
        self.cursor_position.column = column;
    }
}
