use std::fs;
use std::io::Stdout;

use crossterm::cursor;
use crossterm::queue;
use crossterm::style::Attributes;
use crossterm::style::Color;
use crossterm::style::ContentStyle;
use crossterm::style::Print;
use crossterm::style::SetStyle;


#[derive(Clone, Copy)]
pub struct Cell {
    pub character: char,
    pub style : ContentStyle,
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        return self.character == other.character && self.style == other.style;
    }
}

impl Cell {
    pub fn new(character: char, fg: Color, bg: Color) -> Cell {
        Cell { character, style: ContentStyle{foreground_color: Some(fg), background_color: Some(bg), underline_color: None,attributes: Attributes::default() }}
    }
    pub fn with_style(character: char, style: ContentStyle) -> Cell {
        Cell { character, style}
    }
}

pub struct Buffer {
    pub data: Vec<Cell>,
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

    pub fn put_cell(&mut self, c: Cell, pos: (usize, usize)) {
        self.data[pos.1 * self.width + pos.0] = c;
    }

    pub fn put_str(&mut self, data: &str, pos: (usize, usize), style: ContentStyle) {
        //TODO deal with lines that are too big for buffer, do we wrap or do we scroll sideways? If so we need to know where to wrap, that also complicates cursor stuff
        for (i, c) in data.chars().enumerate() {
            if pos.0 + i >= self.width {break}; // Don't render anything that isn't going to be seen
            let index = pos.1 * self.width + pos.0 + i;
            self.data[index] = Cell::with_style(c, style);
        }
    }

    pub fn put_diff(&mut self, stdout: &mut Stdout, other: &Buffer) -> anyhow::Result<()> {
        assert!(self.width == other.width && self.height == other.height);
        queue!(stdout, cursor::Hide)?;
        for (index, cells) in self
            .data
            .iter()
            .zip(other.data.iter())
            .enumerate()
            .filter(|(_, (curr_cell, other_cell))| **curr_cell != **other_cell)
        {
            let (_, other_cell) = cells;
            let x = (index % self.width).try_into().unwrap();
            let y = (index / self.width).try_into().unwrap();
            queue!(stdout, cursor::MoveTo(x, y))?;

            // queue!(stdout, crossterm::style::pr
            queue!(stdout, SetStyle(other_cell.style))?;
            // queue!(stdout, SetForegroundColor(other_cell.fg))?;
            // queue!(stdout, SetBackgroundColor(other_cell.bg))?;
            queue!(stdout, Print(other_cell.character))?;
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
    pub bytes_len : usize,
}

impl TextBuffer {
    pub fn from_path(path: &str) -> anyhow::Result<TextBuffer> {
        let str = fs::read_to_string(path)?;
        let lines: Vec<String> = str.split('\n').map(|slice| slice.trim().to_string()).collect();
        let bytes_len = lines.iter().map(|s| s.len() + 1 ).reduce(|acc, e| acc + e).unwrap_or(1) - 1;
        Ok(TextBuffer {
            lines: lines,
            path: path.to_owned(),
            bytes_len : bytes_len,
        })
    }

    pub fn new(path: &str) -> TextBuffer {
        TextBuffer {
            lines: Vec::new(),
            path: path.to_owned(),
            bytes_len : 0,
        }
    }
}
