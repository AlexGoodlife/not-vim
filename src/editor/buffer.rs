use std::fs;
use std::fs::OpenOptions;
use std::io::Write;

use crossterm::cursor;
use crossterm::queue;
use crossterm::style::Attributes;
use crossterm::style::Color;
use crossterm::style::ContentStyle;
use crossterm::style::Print;
use crossterm::style::SetStyle;

#[derive(Clone)]
pub struct Viewport {
    pub pos: (usize, usize),
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Copy)]
pub struct Cell {
    pub character: char,
    pub style: ContentStyle,
}

impl PartialEq for Cell {
    fn eq(&self, other: &Self) -> bool {
        return self.character == other.character && self.style == other.style;
    }
}

impl Cell {
    pub fn new(character: char, fg: Color, bg: Color) -> Cell {
        Cell {
            character,
            style: ContentStyle {
                foreground_color: Some(fg),
                background_color: Some(bg),
                underline_color: None,
                attributes: Attributes::default(),
            },
        }
    }
    pub fn with_style(character: char, style: ContentStyle) -> Cell {
        Cell { character, style }
    }
}

#[derive(Debug)]
pub struct BufferDiff {
    pub content: String,
    pub pos: (usize, usize),
    pub style: ContentStyle,
}

pub struct RenderBuffer {
    pub data: Vec<Cell>,
    pub width: usize,
    pub height: usize,
}

impl RenderBuffer {
    pub fn new(width: usize, height: usize) -> RenderBuffer {
        RenderBuffer {
            data: vec![Cell::new(' ', Color::Red, Color::Red); width * height],
            width,
            height,
        }
    }

    pub fn put_cell(&mut self, c: Cell, pos: (usize, usize)) {
        let index = pos.1 * self.width + pos.0;
        if index < self.data.len() {
            self.data[pos.1 * self.width + pos.0] = c;
        }
    }

    pub fn put_cells(&mut self, cells: &Vec<Cell>, pos: (usize, usize), viewport: &Viewport) {
        for (i, c) in cells.iter().enumerate() {
            let x = std::cmp::min(self.width - 1, pos.0 + viewport.pos.0);
            let y = std::cmp::min(self.height - 1, pos.1 + viewport.pos.1);
            if x + i >= self.width || x + i >= x + viewport.width {
                break;
            }; // Don't render anything that isn't going to be seen
            let index = y * self.width + x + i;
            self.data[index] = *c;
        }
    }

    pub fn put_str(
        &mut self,
        data: &str,
        pos: (usize, usize),
        style: ContentStyle,
        viewport: &Viewport,
    ) {
        //TODO deal with lines that are too big for buffer, do we wrap or do we scroll sideways? If so we need to know where to wrap, that also complicates cursor stuff
        for (i, c) in data.chars().enumerate() {
            let x = std::cmp::min(self.width - 1, pos.0 + viewport.pos.0);
            let y = std::cmp::min(self.height - 1, pos.1 + viewport.pos.1);
            if x + i >= self.width || x + i >= x + viewport.width {
                break;
            }; // Don't render anything that isn't going to be seen
            let index = y * self.width + x + i;
            self.data[index] = Cell::with_style(c, style);
        }
    }
    #[deprecated(note = "please use `diff` instead")]
    pub fn put_diff(
        &mut self,
        stdout: &mut impl Write,
        other: &RenderBuffer,
    ) -> anyhow::Result<()> {
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

            queue!(stdout, SetStyle(other_cell.style))?;
            queue!(stdout, Print(other_cell.character))?;
        }
        Ok(())
    }

    pub fn diff(&mut self, other: &RenderBuffer) -> Vec<BufferDiff> {
        assert!(self.width == other.width && self.height == other.height);
        let mut result = Vec::new();
        // let diffed_cells: Vec<(usize, (&Cell, &Cell))> = self
        //     .data
        //     .iter()
        //     .zip(other.data.iter())
        //     .enumerate()
        //     .filter(|(_, (curr_cell, other_cell))| **curr_cell != **other_cell)
        //     .collect();
        let mut diffed_cells = Vec::new();

        for i in 0..self.data.len() {
            let curr_cell = self.data[i];
            let other_cell = other.data[i];
            if  curr_cell != other_cell {
                diffed_cells.push((i,(curr_cell,other_cell)));
            }
        }
        // Go along the cells and accumualte cells with same style that are one after the other so
        // we can save on calls to move cursor and set style
        let n = diffed_cells.len();
        let mut i = 0;
        let mut content = String::new();
        while i < n {
            let (index, cells) = diffed_cells[i];
            let (_, new_cell) = cells;
            let style = new_cell.style;
            let x = (index % self.width).try_into().unwrap();
            let y = (index / self.width).try_into().unwrap();
            content.push(new_cell.character);

            let mut j = 1;

            while (i + j) < n {
                let (next_index, next_cells) = diffed_cells[i + j];
                let (_, next_cell) = next_cells;
                let (previous_index, _) = diffed_cells[(i + j) - 1];

                if next_cell.style != style || next_index != previous_index + 1 {
                    break;
                }

                content.push(next_cell.character);
                j += 1;
            }

            result.push(BufferDiff {
                content: content.to_owned(),
                pos: (x, y),
                style: style.to_owned(),
            });
            i += j;
            content.clear();
        }
        result
    }
    // pub fn diff(&mut self, other: &RenderBuffer) -> Vec<BufferDiff> {
    //     assert!(self.width == other.width && self.height == other.height);
    //     self
    //         .data
    //         .iter()
    //         .zip(other.data.iter())
    //         .enumerate()
    //         .filter(|(_, (curr_cell, other_cell))| **curr_cell != **other_cell)
    //         .map(|(index, (_, other_cell))| {
    //             let x = (index % self.width).try_into().unwrap();
    //             let y = (index / self.width).try_into().unwrap();
    //             BufferDiff{
    //                 content: other_cell.character.to_string(),
    //                 pos: (x,y),
    //                 style: other_cell.style,
    //             }
    //         }).collect::<Vec<BufferDiff>>()
    //
    // }

    pub fn copy_into(&mut self, other: &mut RenderBuffer) {
        for (i, cell) in self.data.iter_mut().enumerate() {
            other.data[i] = *cell;
        }
    }

    pub fn clear_buffer(&mut self, bg: Color) {
        for cell in self.data.iter_mut() {
            *cell = Cell::new(' ', bg, bg);
        }
    }
}

pub struct TextBuffer {
    pub lines: Vec<String>,
    pub path: String,
    pub bytes_len: usize,
    pub has_changes: bool,
}

impl TextBuffer {
    pub fn from_path(path: &str) -> anyhow::Result<TextBuffer> {
        let str = fs::read_to_string(path)?;
        let lines: Vec<String> = str
            .split('\n')
            .map(|slice| slice.trim_end().to_string())
            .collect();
        let bytes_len = lines
            .iter()
            .map(|s| s.len() + 1)
            .reduce(|acc, e| acc + e)
            .unwrap_or(1)
            - 1;
        Ok(TextBuffer {
            lines,
            path: path.to_owned(),
            bytes_len,
            has_changes: false,
        })
    }

    pub fn new(path: &str) -> TextBuffer {
        TextBuffer {
            lines: vec![String::new(); 1],
            path: path.to_owned(),
            bytes_len: 0,
            has_changes: false,
        }
    }

    pub fn write_to_file(&mut self) -> anyhow::Result<(usize, usize)> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(self.path.clone())?;

        let binding = self.lines.join("\n");
        let content = binding.as_bytes();
        let n = content.len();
        let mut written = file.write(content)?;
        log::info!("Wrote {} bytes as a chunk", written);
        while n.saturating_sub(written) != 0 {
            written += file.write(&content[written..])?;
            log::info!("Wrote {} bytes as a chunk", written);
        }
        assert!(written == n); // We should really return an error here but for now the assertion
                               // stays
        self.bytes_len = written;
        Ok((written, self.lines.len()))
    }
}
