use crate::editor::buffer::Cell;
use crate::editor::Editor;
use crate::editor::EditorStatus;
use crate::editor::Mode;
use std::io::Stdout;
use std::io::Write;
use std::mem;
use std::time::Duration;

use crate::editor::buffer::Buffer;
use crate::styles::*;
use crossterm::cursor;
use crossterm::event;
use crossterm::event::poll;
use crossterm::event::read;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyEventState;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::queue;
use crossterm::terminal;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;

const DEBUG: bool = false;

pub struct Client {
    stdout: Stdout,
    quit: bool,
    window_dimensions: (u16, u16),
    curr_buffer: Buffer,
    next_buffer: Buffer,
    cursor_pos: (u16, u16),
    top_index: usize,
    left_offset: usize, // For line numbers,
    pub editor: Editor,
}

impl Client {
    pub fn new(stdout: Stdout, dimensions: (u16, u16)) -> Client {
        Client {
            stdout,
            quit: false,
            window_dimensions: (dimensions.0, dimensions.1 - 1), // for gutter
            curr_buffer: Buffer::new(dimensions.0.into(), dimensions.1.into()),
            next_buffer: Buffer::new(dimensions.0.into(), dimensions.1.into()),
            cursor_pos: (0, 0),
            top_index: 0,
            editor: Editor::new(),
            left_offset: 3, // space number |
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        execute!(self.stdout, terminal::EnterAlternateScreen)?;
        execute!(self.stdout, crossterm::cursor::SetCursorStyle::SteadyBlock)?;
        enable_raw_mode()?;
        while !self.quit {
            self.handle_events()?;
            self.update()?;
        }
        Ok(())
    }

    pub fn draw_gutter(&mut self) {
        // Todo refactor all of this bs
        let status = EditorStatus::from_editor(&self.editor);
        // We draw the gutter across the entire buffer
        let mode = status.mode.to_string();
        let mode_len = mode.chars().count();

        let position = format!("{}L {}C ", status.cursor_pos.1, status.cursor_pos.0);
        let position_len = position.chars().count();

        let name = status.curr_buffer;
        let name_len = name.chars().count();

        let bytes = format!("{} B", status.bytes.to_string());
        let bytes_len = bytes.chars().count();

        let spacing_size = 3; // random spaces between things
        let padding_len = self
            .curr_buffer
            .width
            .saturating_sub(mode_len + position_len + name_len + spacing_size + bytes_len);

        let c = "█".repeat(mode_len + 2);

        let y = self.next_buffer.height.saturating_sub(1);
        self.next_buffer
            .put_str(&c, (0, y), gutter_style(&self.editor.mode));
        self.next_buffer
            .put_str(&mode, (1, y), mode_style(&self.editor.mode));
        self.next_buffer
            .put_str(&name, (c.chars().count() + 1, y), default_text_style());

        let c_2 = "█".repeat(
            self.next_buffer
                .width
                .saturating_sub(bytes_len + position_len),
        );

        self.next_buffer.put_str(
            &c_2,
            (mode_len + padding_len + name_len - 1, y),
            gutter_style(&self.editor.mode),
        );


        for (i, c) in position.chars().enumerate() {
            let (char_to_put, style) = match c {
                ' ' => ('█', gutter_style(&self.editor.mode)),
                _ => (c, mode_style(&self.editor.mode)),
            };
            self.next_buffer.put_cell(
                Cell::with_style(char_to_put, style),
                (mode_len + padding_len + name_len + i, y),
            );
        }
        for (i, c) in bytes.chars().enumerate() {
            let (char_to_put, style) = match c {
                ' ' => ('█', gutter_style(&self.editor.mode)),
                _ => (c, mode_style(&self.editor.mode)),
            };
            self.next_buffer.put_cell(
                Cell::with_style(char_to_put, style),
                (mode_len + padding_len + name_len + position_len + i, y),
            );
        }
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
            if i >= self.window_dimensions.1 as usize {
                break;
            }

            self.next_buffer
                .put_str(line, (self.left_offset, i), default_text_style());
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

    fn update(&mut self) -> anyhow::Result<()> {
        self.draw_line_numbers();
        self.update_cursor();
        self.draw_lines();
        self.draw_gutter();
        self.curr_buffer
            .put_diff(&mut self.stdout, &self.next_buffer)?;

        mem::swap(&mut self.next_buffer, &mut self.curr_buffer);

        self.next_buffer.clear_buffer();
        queue!(
            self.stdout,
            cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
        )?;
        queue!(self.stdout, crossterm::cursor::Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    fn handle_insert_keys(&mut self, ev: event::KeyEvent) -> anyhow::Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char(character),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.put_char(character);
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.mode = Mode::Normal;
                queue!(self.stdout, crossterm::cursor::SetCursorStyle::SteadyBlock)?
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.put_newline();
            }
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.pop_backspace();
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_up();
            }
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_down();
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_normal_keys(&mut self, ev: event::KeyEvent) -> anyhow::Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.quit = true;
            }
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_down();
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_up();
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.pop_char();
            }
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.editor.mode = Mode::Insert;
                queue!(self.stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)?
            }
            _ => (),
        }
        Ok(())
    }

    fn handle_events(&mut self) -> anyhow::Result<()> {
        if poll(Duration::from_millis(16))? {
            match read()? {
                Event::Resize(w, h) => {
                    self.next_buffer = Buffer::new(w.into(), h.into());
                    self.curr_buffer = Buffer::new(w.into(), h.into());
                    self.cursor_pos = (0, 0);
                    self.top_index = 0;
                    self.window_dimensions = (w, h - 1); // -1 for gutter
                    self.stdout.flush()?;
                    execute!(
                        self.stdout,
                        cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
                    )?;
                    execute!(self.stdout, terminal::Clear(terminal::ClearType::All))?;
                }
                Event::Key(ev) => match self.editor.mode {
                    Mode::Normal => self.handle_normal_keys(ev)?,
                    Mode::Insert => self.handle_insert_keys(ev)?,
                },
                _ => println!("Some other event"),
            }
        }
        Ok(())
    }

    fn draw_line_numbers(&mut self) {
        self.left_offset = self.editor.buffer.lines.len().to_string().chars().count() + 3; //  3 extra for '|' and a  2 spaces
        for (i, _line) in self
            .editor
            .buffer
            .lines
            .iter()
            .skip(self.top_index)
            .enumerate()
        {
            if i >= self.window_dimensions.1 as usize {
                break;
            }

            let num_str = (i + self.top_index + 1).to_string();
            let padding = self.left_offset - 3;
            let padded = format!("{:>padding$} │ ", num_str);

            self.next_buffer
                .put_str(&padded, (0, i), default_line_number_style());
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.stdout, terminal::Clear(terminal::ClearType::All)).unwrap();
        execute!(self.stdout, terminal::LeaveAlternateScreen).unwrap();
        if DEBUG {
            // let mut i = 0;
            // for ele in self.editor.buffer.lines.clone() {
            //     log::info!("{ele}");
            //     i = i + 1;
            //     if i == 10{
            //         break;
            //     }
            // }
            let mut i = 0;
            for cell in self.curr_buffer.data.clone() {
                if cell.character == ' ' {
                    continue;
                }
                if cell.character == '\n' {
                    log::info!("NEWLINE");
                    continue;
                }
                if cell.character == '\r' {
                    log::info!("CARRIAGE RETURN");
                    continue;
                }
                log::info!("{}", cell.character);
                i = i + 1;
                if i == 100 {
                    break;
                }
            }
        }
    }
}
