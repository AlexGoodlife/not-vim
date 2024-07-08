pub mod buffer;

use crate::editor::buffer::TextBuffer;

const DEFAULT_FILE_PATH: &str = "default.txt";

#[derive(PartialEq,Clone)]
pub enum Mode {
    Normal,
    Insert,
}

impl Mode{
    pub fn to_string(&self) -> String{
        match self {
            Self::Normal => "NORMAL".to_string(),
            Self::Insert => "INSERT".to_string(),
        }
    }
}


pub struct EditorStatus{
    pub cursor_pos : (usize, usize),
    pub curr_buffer : String,
    pub mode : Mode,
    pub bytes : usize,
}

impl EditorStatus{
    pub fn from_editor(editor: &Editor) -> EditorStatus{
        EditorStatus{
            cursor_pos : editor.cursor_pos,
            curr_buffer : editor.buffer.path.to_string(),
            mode : editor.mode.clone(),
            bytes : editor.buffer.bytes_len,
        }
    }
}

pub struct Editor {
    pub buffer: TextBuffer,
    pub cursor_pos: (usize, usize), // x, y, collumn, rows
    pub mode: Mode,
    pub message : String,
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            buffer: TextBuffer::new(DEFAULT_FILE_PATH),
            cursor_pos: (0, 0),
            mode: Mode::Normal,
            message: String::new(),
        }
    }

    pub fn open_file(&mut self, path: &str) -> anyhow::Result<()> {
        self.buffer = TextBuffer::from_path(path)?;
        log::info!("{}", self.buffer.lines.len());
        Ok(())
    }

    pub fn write_current_buffer(&mut self) -> anyhow::Result<()>{
        let (bytes, n ) = self.buffer.write_to_file()?;
        self.message = format!("Wrote {} lines and {} bytes into \"{}\"", n, bytes, self.buffer.path);
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
        self.cursor_pos.0 += 1;
    }

    pub fn put_newline(&mut self) {
        let curr_line = &mut self.buffer.lines[self.cursor_pos.1];
        let rest_of_str: String = curr_line
            .chars()
            .skip(self.cursor_pos.0)
            .skip_while(|c| *c == ' ')
            .collect();

        *curr_line = curr_line
            .chars()
            .enumerate()
            .take_while(|(i, _)| *i < self.cursor_pos.0)
            .map(|(_, c)| c)
            .collect();
        self.buffer.lines.insert(self.cursor_pos.1 + 1, rest_of_str);
        self.cursor_pos.1 += 1;
        self.cursor_pos.0 = 0;
    }

    pub fn pop_backspace(&mut self) {
        //TODO fix the weird skipping issue, or just make the cursor be more leniant to being
        //outside the buffer
        let prev_pos = self.cursor_pos;
        self.move_cursor_left();
        let new_pos = self.cursor_pos;
        if new_pos.0 == prev_pos.0 {
            // We actually want to join the two lines together
            let first_line = self.cursor_pos.1;
            let second_line = self.cursor_pos.1.checked_sub(1).unwrap_or(0);
            let second_line_cursor_pos = self.buffer.lines[second_line].chars().count();
            self.join_lines(second_line, first_line);
            self.cursor_pos.0 = second_line_cursor_pos;
            self.cursor_pos.1 = self.cursor_pos.1.checked_sub(1).unwrap_or(0);
        } else {
            self.pop_char();
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

                let value_to_sub = match self.mode == Mode::Normal {
                    //Normal mode can go a little bit out of the buffer
                    true => 1,
                    false => 0,
                };

                if line.len() > 0 && self.cursor_pos.0 > line.chars().count() - value_to_sub {
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
        let value_to_sub = match self.mode == Mode::Normal {
            //Normal mode can go a little bit out of the buffer
            true => 1,
            false => 0,
        };
        self.cursor_pos.0 = std::cmp::min(
            self.cursor_pos.0 + 1,
            self.buffer.lines[self.cursor_pos.1]
                .chars()
                .count()
                .checked_sub(value_to_sub)
                .unwrap_or(0),
        );
    }

    pub fn move_cursor_down(&mut self) {
        self.cursor_pos.1 = std::cmp::min(self.cursor_pos.1 + 1, self.buffer.lines.len() - 1);
        if self.cursor_pos.1 != self.buffer.lines.len()  {
            // If we are not in the very last line
            self.cursor_pos.0 = std::cmp::min(
                self.buffer.lines[self.cursor_pos.1]
                    .chars()
                    .count()
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
                    .chars()
                    .count()
                    .checked_sub(1)
                    .unwrap_or(0),
                self.cursor_pos.0,
            );
        }
    }

    fn join_lines(&mut self, first_line: usize, second_line: usize) {
        if first_line == second_line {
            return;
        };
        let mut first_string = self.buffer.lines[first_line].to_string();
        first_string.push_str(self.buffer.lines[second_line].as_str());

        log::info!("{}", first_string);
        self.buffer.lines[first_line] = first_string;
        self.buffer.lines.remove(second_line);
    }

}
