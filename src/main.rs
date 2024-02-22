//! Minesweeper game

use std::cmp::min;
use std::io::{stdout, ErrorKind, Write};

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::tty::IsTty;
use crossterm::{
    execute, queue,
    style::{Color, Print, PrintStyledContent, ResetColor, Stylize},
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
            size,
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
            .map(|index| if self.get(index) { 1 } else { 0 })
            .sum()
    }

    fn around(&self, index: IndexPair) -> GridIterator {
        GridIterator::around(self.size, index)
    }
}

struct GridIterator {
    start_index: IndexPair,
    end_index: IndexPair,
    current_index: IndexPair,
}

impl GridIterator {
    fn new(start_index: IndexPair, end_index: IndexPair) -> Self {
        Self {
            start_index,
            end_index,
            current_index: start_index,
        }
    }
    fn all(size: IndexPair) -> Self {
        Self::new(IndexPair { row: 0, col: 0 }, size)
    }

    fn around(size: IndexPair, index: IndexPair) -> Self {
        let start_index = IndexPair {
            row: index.row.saturating_sub(1),
            col: index.col.saturating_sub(1),
        };
        let end_index = IndexPair {
            row: min(index.row + 2, size.row),
            col: min(index.col + 2, size.col),
        };
        Self::new(start_index, end_index)
    }
}

impl Iterator for GridIterator {
    type Item = IndexPair;

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
        Some(index)
    }
}

struct Field {
    size: IndexPair,
    n_mines: u16,
    are_mines_allocated: bool,

    mines: Grid,
    opened: Grid,
    //flags: Grid,
}

struct FieldItem {
    is_opened: bool,
    is_mined: bool,
}

impl Field {
    fn new(size: IndexPair, n_mines: u16) -> Self {
        Self {
            size,
            n_mines,
            are_mines_allocated: false,

            mines: Grid::new(size),
            opened: Grid::new(size),
            //flags: Grid::new(size),
        }
    }

    fn allocate_mines(&mut self) {
        let mut rng = thread_rng();
        for index in sample(&mut rng, self.mines.data.len(), self.n_mines.into()) {
            self.mines.data[index] = true;
        }
        self.are_mines_allocated = true;
    }

    fn handle_click(&mut self, index: IndexPair) {
        if !self.are_mines_allocated {
            self.allocate_mines();
        }
        self.open_at(index);
    }

    fn open_at(&mut self, index: IndexPair) {
        if self.opened.get(index) {
            return;
        }
        self.opened.set(index, true);
        if self.mines.get(index) || self.mines.sum_neighbors(index) > 0 {
            return;
        }
        for index in self.opened.around(index) {
            if !self.opened.get(index) && !self.mines.get(index) {
                self.open_at(index);
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (IndexPair, FieldItem)> + '_ {
        let iterator = GridIterator::all(self.size);
        iterator.map(|index| {
            (
                index,
                FieldItem {
                    is_opened: self.opened.get(index),
                    is_mined: self.mines.get(index),
                },
            )
        })
    }
}

struct GameState {
    field: Field,
    stdout: std::io::Stdout,
    start: IndexPair,
}

impl GameState {
    fn new(size: IndexPair, n_mines: u16) -> Self {
        Self {
            field: Field::new(size, n_mines),
            stdout: stdout(),
            start: IndexPair { row: 1, col: 1 },
        }
    }

    fn handle_key(&mut self, event: &KeyEvent) -> std::io::Result<()> {
        self.report(format!("{:?}", event).as_str())?;
        Ok(())
    }

    fn handle_mouse(&mut self, event: &MouseEvent) -> std::io::Result<()> {
        self.report(format!("{:?}", event).as_str())?;
        if let MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            ..
        } = event
        {
            if let Some(index) = self.convert_absolute_to_relative(IndexPair {
                row: *row,
                col: *column,
            }) {
                self.field.handle_click(index);
            }
        }
        Ok(())
    }

    fn report(&mut self, text: &str) -> std::io::Result<()> {
        queue!(
            self.stdout,
            MoveTo(0, 0),
            Clear(ClearType::CurrentLine),
            Print(text),
        )?;
        Ok(())
    }

    fn draw(&mut self) -> std::io::Result<()> {
        let blue = Color::AnsiValue(21);
        let red = Color::AnsiValue(196);
        let white_opened = Color::AnsiValue(231);
        let grey_opened = Color::AnsiValue(253);
        let white_closed = Color::AnsiValue(48);
        let grey_closed = Color::AnsiValue(41);

        for (
            index,
            FieldItem {
                is_opened,
                is_mined,
            },
        ) in self.field.iter()
        {
            let bg_color = match (is_opened, (index.col + index.row) % 2) {
                (true, 0) => grey_opened,
                (true, 1) => white_opened,
                (false, 0) => grey_closed,
                (false, 1) => white_closed,
                _ => unreachable!(),
            };
            let neighbors = self.field.mines.sum_neighbors(index);
            let content = match (is_opened, is_mined, neighbors) {
                (false, ..) => "  ".to_string().with(bg_color),
                (true, true, ..) => " *".to_string().with(red),
                (true, false, 0) => "  ".to_string().with(bg_color),
                (true, false, ..) => format!(" {}", neighbors).with(blue),
            };
            queue!(
                self.stdout,
                MoveTo(self.start.col + 2 * index.col, self.start.row + index.row),
                PrintStyledContent(content.on(bg_color))
            )?;
        }
        queue!(self.stdout, ResetColor)?;
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stdout.flush()?;
        Ok(())
    }

    fn convert_absolute_to_relative(&self, old_coords: IndexPair) -> Option<IndexPair> {
        if self.start.row <= old_coords.row && self.start.col <= old_coords.col {
            let new_coords = IndexPair {
                row: old_coords.row - self.start.row,
                col: (old_coords.col - self.start.col) / 2,
            };
            if new_coords.row < self.field.size.row && new_coords.col < self.field.size.col {
                return Some(new_coords);
            }
        }

        None
    }
}

fn main() -> std::io::Result<()> {
    if !stdout().is_tty() {
        return Err(std::io::Error::new(ErrorKind::Other, "not a tty!"));
    }

    // setup terminal
    enable_raw_mode()?;
    execute!(stdout(), EnableMouseCapture, EnterAlternateScreen, Hide)?;

    let mut game = GameState::new(IndexPair { row: 10, col: 10 }, 10);

    // event loop
    loop {
        game.draw()?;
        game.flush()?;
        match read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }) => break,
            Event::Key(event) => game.handle_key(&event),
            Event::Mouse(event) => game.handle_mouse(&event),
            _ => continue,
        }?;
    }

    // teardown terminal
    disable_raw_mode()?;
    execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show)?;

    Ok(())
}
