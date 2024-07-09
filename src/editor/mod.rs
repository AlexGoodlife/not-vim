pub mod buffer;

use crate::editor::buffer::TextBuffer;

const DEFAULT_FILE_PATH: &str = "default.txt";
pub(crate) const TABSTOP: usize = 2;

#[derive(PartialEq, Clone)]
pub enum Mode {
    Normal,
    Insert,
}

impl Mode {
    pub fn to_string(&self) -> String {
        match self {
            Self::Normal => "NORMAL".to_string(),
            Self::Insert => "INSERT".to_string(),
        }
    }
}

pub struct EditorStatus {
    pub cursor_pos: (usize, usize),
    pub curr_buffer: String,
    pub mode: Mode,
    pub bytes: usize,
    pub has_changes: bool,
}

impl EditorStatus {
    pub fn from_editor(editor: &Editor) -> EditorStatus {
        EditorStatus {
            cursor_pos: editor.cursor_pos,
            curr_buffer: editor.buffer.path.to_string(),
            mode: editor.mode.clone(),
            bytes: editor.buffer.bytes_len,
            has_changes: editor.buffer.has_changes,
        }
    }
}

pub struct Editor {
    pub buffer: TextBuffer,
    pub cursor_pos: (usize, usize), // x, y, collumn, rows
    pub mode: Mode,
    pub message: String,
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

    pub fn write_current_buffer(&mut self) -> anyhow::Result<()> {
        let (bytes, n) = self.buffer.write_to_file()?;
        self.message = format!(
            "Wrote {} lines and {} bytes into \"{}\"",
            n, bytes, self.buffer.path
        );
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
        self.buffer.has_changes = true;
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
        self.buffer.has_changes = true;
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
            self.cursor_pos.1 = self.cursor_pos.1.checked_sub(1).unwrap_or(0);
            self.cursor_pos.0 = if self.cursor_pos.1 == 0 {
                0
            } else {
                second_line_cursor_pos
            };
        } else {
            self.pop_char();
        }
        self.buffer.has_changes = true;
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
        self.buffer.has_changes = true;
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
                    //Insert mode can go a little bit out of the buffer
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
        self.buffer.has_changes = true;
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
                .saturating_sub(value_to_sub),
        );
    }

    pub fn get_spaces_till_next_tab(index: usize, tabstop: usize) -> usize {
        let tab_stop_index = index / tabstop;
        ((tab_stop_index * tabstop) + tabstop).saturating_sub(index)
    }

    fn get_shiftwidth(s: &str, index: usize, tabstop: usize) -> usize {
        if index == 0 {
            return 0;
        } // we can't possibly shift at index 0
        s.chars()
            .take(index + 1)
            .enumerate()
            .fold(0, |acc: usize, c| {
                let (i, char) = c;
                if char == '\t' {
                    return acc + Self::get_spaces_till_next_tab(acc + i, tabstop) - 1;
                }
                acc
            })
    }
    fn length_with_tabs_at(s: &str, index: usize, tabstop: usize) -> usize {
        Self::get_shiftwidth(s, index, tabstop) + index + 1
    }

    fn length_with_tabs(s: &str, tabstop: usize) -> usize {
        // This is sort of bad performance since we iterate over the string twice but editor
        // strings are usually small so its fine
        Self::length_with_tabs_at(s, s.chars().count().saturating_sub(1), tabstop)
    }

    fn next_line_cursor_index(&mut self, current_y: usize, previous_y: usize) -> usize {
        let normal_len = &self.buffer.lines[current_y].chars().count();
        let len = Self::length_with_tabs(&self.buffer.lines[current_y], TABSTOP);
        let value_to_sub = match self.mode == Mode::Normal {
            //Insert mode can go a little bit out of the buffer
            true => 1,
            false => 0,
        };
        let cursor_x =
            Self::length_with_tabs_at(&self.buffer.lines[previous_y], self.cursor_pos.0, TABSTOP)
                .saturating_sub(1);

        // We need to find the shiftwidth on the cursor_x on the line below us so we can shift
        // accordingly, this is because a line under can have any arbitrary number of \t on any
        // arbitrary index, so finding the correct index to move to is crucial, it behaves both
        // differently to vscode and vim but its fine I think
        let mut shiftwidth = 0;
        let mut i = 0;
        for c in self.buffer.lines[current_y].chars() {
            // log::info!("index {i} {shiftwidth}, i + s{}", i + shiftwidth);
            if c == '\t' {
                let add = Self::get_spaces_till_next_tab(i + shiftwidth, TABSTOP).saturating_sub(1);
                shiftwidth += add;
            }
            if i + shiftwidth >= cursor_x || i > *normal_len {
                break;
            }
            i += 1;
        }
        // log::warn!("s {}, i {}, len {}, px {} cx {}", shiftwidth, i.saturating_sub(value_to_sub), normal_len.saturating_sub(value_to_sub), self.cursor_pos.0, cursor_x);
        std::cmp::min(normal_len.saturating_sub(value_to_sub), i)
    }

    pub fn move_cursor_down(&mut self) {
        let previous_y = self.cursor_pos.1;
        self.cursor_pos.1 = std::cmp::min(self.cursor_pos.1 + 1, self.buffer.lines.len() - 1);
        if self.cursor_pos.1 != self.buffer.lines.len() {
            // If we are not in the very last line
            self.cursor_pos.0 = self.next_line_cursor_index(self.cursor_pos.1, previous_y);
        }
    }

    pub fn move_cursor_up(&mut self) {
        let previous_y = self.cursor_pos.1;
        self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(1);
        if previous_y != 0 {
            self.cursor_pos.0 = self.next_line_cursor_index(self.cursor_pos.1, previous_y);
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
        self.buffer.has_changes = true;
    }
}
