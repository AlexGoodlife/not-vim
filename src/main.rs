use simple_logging::log_to_file;
use std::env;
use std::fs;
use std::io::stdout;
use std::io::Stdout;
use std::io::Write;
use std::mem;
use std::time::Duration;

use crossterm::cursor;
use crossterm::event;
use crossterm::event::poll;
use crossterm::event::read;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::queue;
use crossterm::style::Color;
use crossterm::style::Print;
use crossterm::style::PrintStyledContent;
use crossterm::style::SetBackgroundColor;
use crossterm::style::SetForegroundColor;
use crossterm::terminal;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::Result;

pub mod client;
pub mod editor;
mod util;

#[derive(Clone, PartialEq, Copy)]
struct Cell {
    character: char,
    fg: Color,
    bg: Color,
}

impl Cell {
    pub fn new(character: char, fg: Color, bg: Color) -> Cell {
        Cell { character, fg, bg }
    }
}

struct Buffer {
    data: Vec<Cell>,
    width: usize,
    height: usize,
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
        let npos = (pos.0 as usize, pos.1 as usize);
        for (i, c) in data.chars().enumerate() {
            self.data[npos.1 * self.width + npos.0 + i] = Cell::new(c, fg, bg);
        }
    }

    pub fn put_diff(&mut self, stdout: &mut Stdout, other: &Buffer) -> Result<()> {
        assert!(self.width == other.width && self.height == other.height);
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

enum Mode {
    Normal,
    Insert,
}
struct Client {
    stdout: Stdout,
    quit: bool,
    lines: Vec<String>,
    window_dimensions: (u16, u16),
    curr_buffer: Buffer,
    next_buffer: Buffer,
    cursor_pos: (u16, u16),
    top_index: usize,
    mode: Mode,
}

impl Client {
    pub fn new(stdout: Stdout, dimensions: (u16, u16), lines: Vec<&str>) -> Client {
        Client {
            stdout: stdout,
            quit: false,
            lines: lines.iter().map(|s| s.to_string()).collect(),
            window_dimensions: dimensions,
            curr_buffer: Buffer::new(dimensions.0.into(), dimensions.1.into()),
            next_buffer: Buffer::new(dimensions.0.into(), dimensions.1.into()),
            cursor_pos: (0, 0),
            top_index: 0,
            mode: Mode::Normal,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        execute!(self.stdout, terminal::EnterAlternateScreen)?;
        enable_raw_mode()?;
        while !self.quit {
            self.update()?;
            self.handle_events()?;
        }
        Ok(())
    }

    pub fn load_lines(&mut self) {
        for (i, line) in self.lines.iter().skip(self.top_index).enumerate() {
            if i >= self.next_buffer.height {
                break;
            }
            self.next_buffer
                .put_str(line, (0, i as u16), Color::White, Color::Reset);
        }
    }

    fn update(&mut self) -> Result<()> {
        self.load_lines();
        self.curr_buffer
            .put_diff(&mut self.stdout, &self.next_buffer)?;
        self.next_buffer.copy_into(&mut self.curr_buffer);
        mem::swap(&mut self.next_buffer, &mut self.curr_buffer);

        self.next_buffer.clear_buffer();
        queue!(
            self.stdout,
            cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    fn handle_insert_keys(&mut self, ev: event::KeyEvent) -> Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char(character),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            } => {
                self.lines[self.top_index as usize + self.cursor_pos.1 as usize]
                    .insert(self.cursor_pos.0 as usize, character);
                self.cursor_pos.0 += 1;
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            } => {
                self.mode = Mode::Normal;
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_normal_keys(&mut self, ev: event::KeyEvent) -> Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
            } => {
                self.quit = true;
            }
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
            } => {
                if (self.cursor_pos.1 + 1) as usize >= self.curr_buffer.height {
                    self.top_index += 1;
                } else {
                    self.cursor_pos.1 += 1;
                }
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
            } => {
                if self.cursor_pos.1 == 0 {
                    self.top_index = self.top_index.checked_sub(1).unwrap_or(0);
                } else {
                    self.cursor_pos.1 -= 1;
                }
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.cursor_pos.0 -= 1;
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.cursor_pos.0 += 1;
            }
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.mode = Mode::Insert;
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if poll(Duration::from_millis(16))? {
            match read()? {
                Event::Key(ev) => match self.mode {
                    Mode::Normal => self.handle_normal_keys(ev)?,
                    Mode::Insert => self.handle_insert_keys(ev)?,
                },
                _ => println!("Some other event"),
            }
        }
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.stdout, terminal::Clear(terminal::ClearType::All)).unwrap();
        execute!(self.stdout, terminal::LeaveAlternateScreen).unwrap();
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    log_to_file("editor.log", log::LevelFilter::Info).unwrap();

    let stdout = stdout();

    let dimensions = terminal::size().unwrap();
    log::info!("Started editor");
    let str = fs::read_to_string("bible.txt").unwrap();
    let lines: Vec<&str> = str.split('\n').collect();
    let mut client = Client::new(stdout, dimensions, lines);
    let _ = client.run().map_err(|err| eprint!("{err}"));
}
