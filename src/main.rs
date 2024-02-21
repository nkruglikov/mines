//! Minesweeper game

use std::cmp::min;
use std::io::{stdout, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{
    execute, queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use rand::prelude::*;
use rand::seq::index::sample;

#[derive(Copy, Clone)]
struct IndexPair {
    row: u16,
    col: u16,
}

struct Grid {
    data: Vec<bool>,
    size: IndexPair,
}

impl Grid {
    fn new(size: IndexPair) -> Self {
        Self {
            data: vec![false; (size.row * size.col).into()],
            size: size,
        }
    }

    fn get(&self, index: IndexPair) -> bool {
        self.data[(index.row * self.size.col + index.col) as usize]
    }

    fn set(&mut self, index: IndexPair, value: bool) {
        self.data[(index.row * self.size.col + index.col) as usize] = value
    }

    fn sum_neighbors(&self, index: IndexPair) -> u16 {
        self.around(index)
            .map(|(_, value)| if value { 1 } else { 0 })
            .sum()
    }

    fn around(&self, index: IndexPair) -> GridIterator {
        GridIterator::around(&self, index)
    }
}

struct GridIterator<'a> {
    grid: &'a Grid,
    start_index: IndexPair,
    end_index: IndexPair,
    current_index: IndexPair,
}

impl<'a> GridIterator<'a> {
    fn new(grid: &'a Grid, start_index: IndexPair, end_index: IndexPair) -> Self {
        Self {
            grid: grid,
            start_index: start_index,
            end_index: end_index,
            current_index: start_index,
        }
    }
    fn all(grid: &'a Grid) -> Self {
        Self::new(grid, IndexPair { row: 0, col: 0 }, grid.size)
    }

    fn around(grid: &'a Grid, index: IndexPair) -> Self {
        let start_index = IndexPair {
            row: index.row.saturating_sub(1),
            col: index.col.saturating_sub(1),
        };
        let end_index = IndexPair {
            row: min(index.row + 2, grid.size.row),
            col: min(index.col + 2, grid.size.col),
        };
        Self::new(grid, start_index, end_index)
    }
}

impl Iterator for GridIterator<'_> {
    type Item = (IndexPair, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index.row >= self.end_index.row {
            return None;
        }
        let index = self.current_index;
        self.current_index.col += 1;
        if self.current_index.col >= self.end_index.col {
            self.current_index.col = self.start_index.col;
            self.current_index.row += 1;
        }
        return Some((index, self.grid.get(index)));
    }
}

struct GameState {
    mines: Grid,
    opened: Grid,
    stdout: std::io::Stdout,
    start: IndexPair,
}

impl GameState {
    fn new(size: IndexPair, n_mines: u16) -> Self {
        let mut result = Self {
            mines: Grid::new(size),
            opened: Grid::new(size),
            stdout: stdout(),
            start: IndexPair { row: 1, col: 1 },
        };
        let mut rng = thread_rng();
        for index in sample(&mut rng, result.mines.data.len(), n_mines.into()) {
            result.mines.data[index] = true;
        }
        result
    }

    fn handle_key(&mut self, event: &KeyEvent) {
        self.report(format!("{:?}", event).as_str());
    }

    fn handle_mouse(&mut self, event: &MouseEvent) {
        self.report(format!("{:?}", event).as_str());
        if let MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row: row,
            ..
        } = event
        {
            if let Some(index) = self.convert_absolute_to_relative(IndexPair {
                row: *row,
                col: *col,
            }) {
                open_field(&mut self.opened, &self.mines, index);
            }
        }
    }

    fn report(&mut self, text: &str) {
        queue!(
            self.stdout,
            MoveTo(0, 0),
            Clear(ClearType::CurrentLine),
            Print(text),
        );
    }

    fn draw(&mut self) {
        let black = Color::AnsiValue(16);
        let blue = Color::AnsiValue(21);
        let red = Color::AnsiValue(196);
        let white_opened = Color::AnsiValue(231);
        let grey_opened = Color::AnsiValue(253);
        let white_closed = Color::AnsiValue(48);
        let grey_closed = Color::AnsiValue(41);

        for row in 0..self.mines.size.row {
            queue!(self.stdout, MoveTo(self.start.col, self.start.row + row));
            for col in 0..self.mines.size.col {
                let index = IndexPair { row: row, col: col };
                if !self.opened.get(index) {
                    let bg_color = if (col + row) % 2 == 0 {
                        grey_closed
                    } else {
                        white_closed
                    };
                    queue!(self.stdout, SetBackgroundColor(bg_color), Print("  "));
                    continue;
                }
                let bg_color = if (col + row) % 2 == 0 {
                    grey_opened
                } else {
                    white_opened
                };
                queue!(self.stdout, SetBackgroundColor(bg_color));
                if self.mines.get(index) {
                    queue!(self.stdout, SetForegroundColor(red), Print(" X"));
                } else {
                    let n = self.mines.sum_neighbors(index);
                    let msg = if n > 0 {
                        format!(" {}", n)
                    } else {
                        "  ".to_string()
                    };
                    queue!(self.stdout, SetForegroundColor(blue), Print(msg.as_str()));
                }
            }
        }
        queue!(self.stdout, ResetColor);
    }

    fn flush(&mut self) {
        let _ = self.stdout.flush();
    }

    fn convert_absolute_to_relative(&self, old_coords: IndexPair) -> Option<IndexPair> {
        if self.start.row <= old_coords.row && self.start.col <= old_coords.col {
            let new_coords = IndexPair {
                row: old_coords.row - self.start.row,
                col: (old_coords.col - self.start.col) / 2,
            };
            if new_coords.row < self.mines.size.row && new_coords.col < self.mines.size.col {
                return Some(new_coords);
            }
        }

        None
    }
}

fn open_field(opened: &mut Grid, mines: &Grid, index: IndexPair) {
    if opened.get(index) {
        return;
    }
    opened.set(index, true);
    if mines.get(index) || mines.sum_neighbors(index) > 0 {
        return;
    }
    for r in index.row.saturating_sub(1)..=min(index.row + 1, opened.size.col - 1) {
        for c in index.col.saturating_sub(1)..=min(index.col + 1, opened.size.row - 1) {
            let index = IndexPair { row: r, col: c };
            if !opened.get(index) && !mines.get(index) {
                open_field(opened, mines, index);
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    // setup terminal
    enable_raw_mode()?;
    execute!(stdout(), EnableMouseCapture, EnterAlternateScreen, Hide)?;

    let mut game = GameState::new(IndexPair { row: 10, col: 10 }, 10);

    // event loop
    loop {
        game.draw();
        game.flush();
        match read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => break,
            Event::Key(event) => game.handle_key(&event),
            Event::Mouse(event) => game.handle_mouse(&event),
            _ => continue,
        }
    }

    // teardown terminal
    disable_raw_mode()?;
    execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show)?;

    Ok(())
}
