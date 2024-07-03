
use std::fs;
use std::io::Stdout;

use crossterm::cursor;
use crossterm::queue;
use crossterm::style::Color;
use crossterm::style::Print;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetForegroundColor;
use crossterm::Result;


#[derive(Clone, PartialEq, Copy)]
pub struct Cell {
    character: char,
    fg: Color,
    bg: Color,
}

impl Cell {
    pub fn new(character: char, fg: Color, bg: Color) -> Cell {
        Cell { character, fg, bg }
    }
}

pub struct Buffer {
    data: Vec<Cell>,
    pub width: usize,
    pub height: usize,
}

impl Buffer {
    pub fn new(width: usize, height: usize) -> Buffer {
        Buffer {
            data: vec![Cell::new(' ', Color::Reset, Color::Reset); width * height],
            width,
            height,
        }
    }

    pub fn dump_buffer(&self, out_stream: &mut Stdout) -> Result<()> {
        for (index, cell) in self.data.iter().enumerate() {
            queue!(
                out_stream,
                cursor::MoveTo(
                    (index % self.width).try_into().unwrap(),
                    (index / self.width).try_into().unwrap()
                )
            )?;
            queue!(out_stream, SetForegroundColor(cell.fg))?;
            queue!(out_stream, SetBackgroundColor(cell.bg))?;
            queue!(out_stream, Print(cell.character))?;
        }
        Ok(())
    }

    pub fn put_cell(&mut self, c: Cell, pos: (u16, u16)) {
        let npos = (pos.0 as usize, pos.1 as usize);
        self.data[npos.1 * self.width + npos.0] = c;
    }

    pub fn put_str(&mut self, data: &str, pos: (u16, u16), fg: Color, bg: Color) {
        //TODO deal with lines that are too big for buffer, do we wrap? If so we need to know where to wrap, that also complicates cursor stuff
        let npos = (pos.0 as usize, pos.1 as usize);
        for (i, c) in data.chars().enumerate() {
            self.data[npos.1 * self.width + npos.0 + i] = Cell::new(c, fg, bg);
        }
    }

    pub fn put_diff(&mut self, stdout: &mut Stdout, other: &Buffer) -> Result<()> {
        assert!(self.width == other.width && self.height == other.height);
        queue!(stdout, cursor::Hide)?;
        for (index, curr_cell) in self.data.iter().enumerate() {
            let other_cell = &other.data[index];
            if curr_cell != other_cell {
                queue!(
                    stdout,
                    cursor::MoveTo(
                        (index % self.width).try_into().unwrap(),
                        (index / self.width).try_into().unwrap()
                    )
                )?;
                queue!(stdout, SetForegroundColor(other_cell.fg))?;
                queue!(stdout, SetBackgroundColor(other_cell.bg))?;
                queue!(stdout, Print(other_cell.character))?;
            }
        }
        Ok(())
    }

    pub fn copy_into(&mut self, other: &mut Buffer) {
        for (i, cell) in self.data.iter_mut().enumerate() {
            other.data[i] = *cell;
        }
    }

    pub fn clear_buffer(&mut self) {
        for cell in self.data.iter_mut() {
            *cell = Cell::new(' ', Color::Reset, Color::Reset);
        }
    }
}

pub struct TextBuffer {
    pub lines: Vec<String>,
    pub path: String,
}

impl TextBuffer {
    pub fn from_path(path: &str) -> Result<TextBuffer> {
        let str = fs::read_to_string(path)?;
        Ok(TextBuffer {
            lines: str.split('\n').map(|slice| slice.to_string()).collect(),
            path: path.to_owned(),
        })
    }

    pub fn new(path: &str) -> TextBuffer {
        TextBuffer {
            lines: Vec::new(),
            path: path.to_owned(),
        }
    }
}
