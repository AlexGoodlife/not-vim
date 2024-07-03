pub mod buffer;

use crate::editor::buffer::TextBuffer;
use crossterm::Result;

const DEFAULT_FILE_PATH: &str = "default.txt";

pub struct Editor {
    pub buffer: TextBuffer,
    pub cursor_pos: (usize, usize),
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            buffer: TextBuffer::new(DEFAULT_FILE_PATH),
            cursor_pos: (0, 0),
        }
    }

    pub fn open_file(&mut self, path: &str) -> Result<()> {
        self.buffer = TextBuffer::from_path(path)?;
        log::info!("{}", self.buffer.lines.len());
        Ok(())
    }

    pub fn put_char(&mut self, c: char) {
        let curr_line = &mut self.buffer.lines[self.cursor_pos.1];
        match curr_line.char_indices().nth(self.cursor_pos.0) {
            Some(result) => {
                curr_line.insert(result.0, c);
            }
            None => {
                curr_line.push(c);
            }
        }
        if c == '\n' {
            let rest_of_str: String = curr_line
                .chars()
                .skip(self.cursor_pos.0 + 1)
                .skip_while(|c| *c == ' ')
                .collect();

            *curr_line = curr_line.chars().take_while(|c| *c != '\n').collect();
            self.buffer.lines.insert(self.cursor_pos.1 + 1, rest_of_str);
            self.cursor_pos.1 += 1;
            self.cursor_pos.0 = 0;
        } else {
            self.cursor_pos.0 += 1;
        }
    }

    fn remove_empty_line(&mut self, index: usize) {
        if self.buffer.lines.len() == 1 {
            // We only have 1 empty line, we want to keep ip for a bit
            log::info!("Trying to remove the last line");
            return;
        }
        log::info!("removing empty line");
        self.buffer.lines.remove(index);
        self.move_cursor_up();
    }

    pub fn pop_char(&mut self) {
        let line = &mut self.buffer.lines[self.cursor_pos.1];
        if line.len() == 0 {
            return self.remove_empty_line(self.cursor_pos.1);
        }
        match line.char_indices().nth(self.cursor_pos.0) {
            Some(result) => {
                line.remove(result.0);

                if line.len() > 0 && self.cursor_pos.0 > line.len() - 1 {
                    self.move_cursor_left()
                }
            }
            None => {
                log::warn!(
                    "Tried removing a character that is in a wrong index : {}",
                    self.cursor_pos.0
                );
            }
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos.0 = self.cursor_pos.0.checked_sub(1).unwrap_or(0);
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor_pos.0 = std::cmp::min(
            self.cursor_pos.0 + 1,
            self.buffer.lines[self.cursor_pos.1]
                .len()
                .checked_sub(1)
                .unwrap_or(0),
        );
    }

    pub fn move_cursor_down(&mut self) {
        self.cursor_pos.1 = std::cmp::min(self.cursor_pos.1 + 1, self.buffer.lines.len() - 1);
        if self.cursor_pos.1 != self.buffer.lines.len() - 1 {
            // If we are not in the very last line
            self.cursor_pos.0 = std::cmp::min(
                self.buffer.lines[self.cursor_pos.1]
                    .len()
                    .checked_sub(1)
                    .unwrap_or(0),
                self.cursor_pos.0,
            );
        }
    }

    pub fn move_cursor_up(&mut self) {
        self.cursor_pos.1 = self.cursor_pos.1.checked_sub(1).unwrap_or(0);
        if self.cursor_pos.1 != 0 {
            // If we are not in the very first line
            self.cursor_pos.0 = std::cmp::min(
                self.buffer.lines[self.cursor_pos.1]
                    .len()
                    .checked_sub(1)
                    .unwrap_or(0),
                self.cursor_pos.0,
            );
        }
    }
}
