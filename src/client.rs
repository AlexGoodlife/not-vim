use crate::editor::Editor;
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
use crossterm::terminal;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::Result;
use crate::editor::buffer::Buffer;

enum Mode {
    Normal,
    Insert,
}

pub struct Client {
    stdout: Stdout,
    quit: bool,
    window_dimensions: (u16, u16),
    curr_buffer: Buffer,
    next_buffer: Buffer,
    cursor_pos: (u16, u16),
    top_index: usize,
    left_offset: usize, // For line numbers,
    mode: Mode,
    pub editor: Editor,
}

impl Client {
    pub fn new(stdout: Stdout, dimensions: (u16, u16)) -> Client {
        Client {
            stdout,
            quit: false,
            window_dimensions: dimensions,
            curr_buffer: Buffer::new(dimensions.0.into(), dimensions.1.into()),
            next_buffer: Buffer::new(dimensions.0.into(), dimensions.1.into()),
            cursor_pos: (0, 0),
            top_index: 0,
            mode: Mode::Normal,
            editor: Editor::new(),
            left_offset: 3, // space number |
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

    pub fn draw_lines(&mut self) {
        for (i, line) in self
            .editor
            .buffer
            .lines
            .iter()
            .skip(self.top_index)
            .enumerate()
        {
            if i >= self.next_buffer.height {
                break;
            }
            self.next_buffer.put_str(
                line,
                (self.left_offset as u16, i as u16),
                Color::White,
                Color::Reset,
            );
        }
    }

    fn update_cursor(&mut self) {
        let (editor_x, editor_y) = self.editor.cursor_pos;
        // let (client_x, client_y) = self.cursor_pos;
        let viewport_height = (self.window_dimensions.1 as usize)
            .checked_sub(1)
            .unwrap_or(0);
        if editor_y >= viewport_height + self.top_index {
            // We need to scroll down
            self.top_index += editor_y - (viewport_height + self.top_index);
        }
        if editor_y < self.top_index {
            // We need to scroll up
            self.top_index -= self.top_index - editor_y;
        }
        self.cursor_pos.0 = self.left_offset as u16 + editor_x as u16;
        self.cursor_pos.1 = (editor_y - self.top_index) as u16;
    }

    fn update(&mut self) -> Result<()> {
        self.draw_line_numbers();
        self.draw_lines();
        self.curr_buffer
            .put_diff(&mut self.stdout, &self.next_buffer)?;
        self.next_buffer.copy_into(&mut self.curr_buffer);
        mem::swap(&mut self.next_buffer, &mut self.curr_buffer);

        self.next_buffer.clear_buffer();
        self.update_cursor();
        // log::info!("{},{}", self.editor.cursor_pos.0, self.editor.cursor_pos.1);
        queue!(
            self.stdout,
            cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
        )?;
        queue!(self.stdout, crossterm::cursor::Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    fn handle_insert_keys(&mut self, ev: event::KeyEvent) -> Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char(character),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            } => {
                self.editor.put_char(character);
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            } => {
                self.mode = Mode::Normal;
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            } => {
                self.editor.put_char('\n');
            }
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
            } => {
                self.editor.move_cursor_left(); // if we are deleting the final character of a what we really are doing is joining two lines together
                self.editor.pop_char();
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
                self.editor.move_cursor_down();
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.editor.move_cursor_up();
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.editor.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.editor.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
            } => {
                self.editor.pop_char();
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
                Event::Resize(w, h) => {
                    self.next_buffer = Buffer::new(w.into(), h.into());
                    self.curr_buffer = Buffer::new(w.into(), h.into());
                    self.cursor_pos = (0, 0);
                    self.top_index = 0;
                    self.window_dimensions = (w, h);
                }
                Event::Key(ev) => match self.mode {
                    Mode::Normal => self.handle_normal_keys(ev)?,
                    Mode::Insert => self.handle_insert_keys(ev)?,
                },
                _ => println!("Some other event"),
            }
        }
        Ok(())
    }

    fn draw_line_numbers(&mut self) {
        self.left_offset = self.editor.buffer.lines.len().to_string().chars().count() + 2; //  2 extra for '|' and a space
        log::info!("left offset {}", self.left_offset);
        for (i, _line) in self
            .editor
            .buffer
            .lines
            .iter()
            .skip(self.top_index)
            .enumerate()
        {
            if i >= self.next_buffer.height {
                break;
            }

            let num_str = (i + self.top_index + 1).to_string();
            log::info!(
                "num {} and count {}",
                i + self.top_index + 1,
                num_str.chars().count()
            );
            let padding = self.left_offset- 2;
            let padded = format!("{:>padding$}| ", num_str);

            self.next_buffer
                .put_str(&padded, (0, i as u16), Color::White, Color::Reset);
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.stdout, terminal::Clear(terminal::ClearType::All)).unwrap();
        execute!(self.stdout, terminal::LeaveAlternateScreen).unwrap();
    }
}
