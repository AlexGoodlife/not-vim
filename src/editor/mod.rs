pub mod buffer;

use std::error::Error;

use crate::editor::buffer::TextBuffer;
use copypasta::{wayland_clipboard::Clipboard, ClipboardContext, ClipboardProvider};

pub type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync + 'static>>;
const DEFAULT_FILE_PATH: &str = "default.txt";
pub(crate) const TABSTOP: usize = 2;

pub fn is_seperator(c: char) -> bool {
    !c.is_alphanumeric() || c.is_whitespace()
}

struct DefaultClipboard {
    data: Vec<String>,
}

impl DefaultClipboard {
    pub fn new() -> Self {
        DefaultClipboard { data: Vec::new() }
    }
}

impl ClipboardProvider for DefaultClipboard {
    fn get_contents(&mut self) -> Result<String> {
        Ok(self.data.join("\n"))
    }

    fn set_contents(&mut self, contents: String) -> Result<()> {
        self.data.clear();
        let split = contents.split('\n');
        for s in split.into_iter() {
            self.data.push(s.to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveInfo {
    pub start_pos: (usize, usize),
    pub end_pos: (usize, usize),
}

impl MoveInfo {
    pub fn is_backwards(&self) -> bool {
        self.start_pos.1 > self.end_pos.1
            || (self.start_pos.1 == self.end_pos.1 && self.start_pos.0 >= self.end_pos.0)
    }

    // as in, start pos is before end pos
    pub fn get_ordered(&self) -> Self {
        if self.is_backwards() {
            MoveInfo {
                start_pos: self.end_pos,
                end_pos: self.start_pos,
            }
        } else {
            self.clone()
        }
    }

    pub fn expand_or_shrink(&self, x: usize, y: usize) -> MoveInfo {
        if x <= self.start_pos.0 && y <= self.start_pos.1 {
            return MoveInfo {
                start_pos: (x, y),
                end_pos: self.end_pos,
            }
            .get_ordered();
        } else {
            return MoveInfo {
                start_pos: self.start_pos,
                end_pos: (x, y),
            }
            .get_ordered();
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
}

impl Mode {
    pub fn to_string(&self) -> String {
        match self {
            Self::Normal => "NORMAL".to_string(),
            Self::Insert => "INSERT".to_string(),
            Self::Visual => "VISUAL".to_string(),
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
            curr_buffer: {
                match &editor.open_buffers[editor.current_index].buffer.path {
                    Some(path) => path.to_string(),
                    None => "Unnamed".to_string(),
                }
            },
            mode: editor.mode.clone(),
            bytes: editor.open_buffers[editor.current_index].buffer.bytes_len,
            has_changes: editor.open_buffers[editor.current_index].buffer.has_changes,
        }
    }
}

struct TextBufferInfo {
    buffer: TextBuffer,
    cursor_pos: (usize, usize),
}

pub struct Editor {
    open_buffers: Vec<TextBufferInfo>,
    current_index: usize,
    pub cursor_pos: (usize, usize), // x, y, collumn, rows
    pub mode: Mode,
    pub message: String,
    pub curr_selection: Option<((usize, usize), MoveInfo)>, // Selection for visual mode, we put the
    // starting cursor position and its selection
    latest_x: Option<usize>, //to make scrolling lines better
    clipboard: Box<dyn ClipboardProvider>,
    has_empty: bool, // to check if the only open buffer is an empty one
}

impl Editor {
    pub fn new() -> Editor {
        let default_clipboard = DefaultClipboard::new();
        let clipboard: Box<dyn ClipboardProvider> = match ClipboardContext::new() {
            Ok(c) => Box::new(c),
            Err(_) => Box::new(default_clipboard),
        };
        let empty = TextBuffer::empty();
        let mut vec = Vec::new();
        vec.push(TextBufferInfo {
            buffer: empty,
            cursor_pos: (0, 0),
        });
        Editor {
            cursor_pos: (0, 0),
            mode: Mode::Normal,
            message: String::new(),
            curr_selection: None,
            latest_x: None,
            clipboard,
            open_buffers: vec,
            current_index: 0, // always gonna have an unnamed buffer
            has_empty: true,
        }
    }

    pub fn open_file(&mut self, path: &str) -> anyhow::Result<()> {
        match self.open_buffers.iter().enumerate().find(|(_, b)| {
            if let Some(p) = &b.buffer.path {
                return path == p;
            }
            return false;
        }) {
            Some((i,b)) => {
                self.current_index = i;
                self.cursor_pos = b.cursor_pos;
            }
            None => {
                let buff = match TextBuffer::from_path(path) {
                    Ok(b) => b,
                    Err(_) => {
                        self.message = "No such file exists or can't be opened".to_string();
                        return Ok(())
                    }
                };
                // meaning there is only 1 buffer
                if self.has_empty &&  self.open_buffers.len() == 1 && !self.curr_buffer().has_changes{
                    self.open_buffers.pop(); // we dont want the empty buffer 
                }
                self.open_buffers.push(TextBufferInfo{buffer: buff, cursor_pos: (0,0)});
                self.cursor_pos = (0,0);
                self.current_index = self.open_buffers.len().saturating_sub(1);
            }
        }
        Ok(())
    }

    pub fn curr_buffer(&self) -> &TextBuffer {
        &self.open_buffers[self.current_index].buffer
    }

    pub fn write_current_buffer(&mut self) -> anyhow::Result<()> {
        let curr_buff = &mut self.open_buffers[self.current_index].buffer;
        if let Some(path) = curr_buff.path.clone() {
            let (bytes, n) = curr_buff.write_to_file(path.as_str())?;
            self.message = format!("Wrote {} lines and {} bytes into \"{}\"", n, bytes, path);
            return Ok(());
        }
        Err(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File requires a path to write",
        )))
    }

    pub fn write_current_buffer_to_path(&mut self, path: &str) -> anyhow::Result<()> {
        let (bytes, n) = self.open_buffers[self.current_index]
            .buffer
            .write_to_file(path)?;
        self.message = format!("Wrote {} lines and {} bytes into \"{}\"", n, bytes, path);
        Ok(())
    }

    pub fn put_char(&mut self, c: char) {
        let curr_buffer = &mut self.open_buffers[self.current_index].buffer;
        let curr_line = &mut curr_buffer.lines[self.cursor_pos.1];
        match curr_line.char_indices().nth(self.cursor_pos.0) {
            Some(result) => {
                curr_line.insert(result.0, c);
            }
            None => {
                curr_line.push(c);
            }
        }
        self.cursor_pos.0 += 1;
        curr_buffer.has_changes = true;
    }

    pub fn put_newline(&mut self) {
        {
            let curr_buffer = &mut self.open_buffers[self.current_index].buffer;
            let curr_line = &mut curr_buffer.lines[self.cursor_pos.1];
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
            curr_buffer.lines.insert(self.cursor_pos.1 + 1, rest_of_str);
            curr_buffer.has_changes = true;
        }
        self.move_cursor_to(0, self.cursor_pos.1 + 1);
    }

    pub fn pop_backspace(&mut self) {
        let prev_pos = self.cursor_pos;
        self.move_cursor_left(1);
        let new_pos = self.cursor_pos;
        if new_pos.0 == prev_pos.0 {
            // We actually want to join the two lines together
            let first_line = self.cursor_pos.1;
            let second_line = self.cursor_pos.1.checked_sub(1).unwrap_or(0);
            let second_line_cursor_pos = self.open_buffers[self.current_index].buffer.lines
                [second_line]
                .chars()
                .count();
            self.join_lines(second_line, first_line);
            if self.cursor_pos.1 != 0 {
                self.move_cursor_to(second_line_cursor_pos, self.cursor_pos.1);
            }
            self.move_cursor_to(
                self.cursor_pos.0,
                self.cursor_pos.1.checked_sub(1).unwrap_or(0),
            );
        } else {
            self.pop_char();
        }
        self.open_buffers[self.current_index].buffer.has_changes = true;
    }

    fn remove_empty_line(&mut self, index: usize) {
        if self.open_buffers[self.current_index].buffer.lines.len() == 1 {
            // We only have 1 empty line, we want to keep ip for a bit
            log::info!("Trying to remove the last line");
            return;
        }
        log::info!("removing empty line");
        self.open_buffers[self.current_index]
            .buffer
            .lines
            .remove(index);
        self.move_cursor_up(1);
        self.open_buffers[self.current_index].buffer.has_changes = true;
    }

    pub fn pop_char(&mut self) {
        let line = &mut self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1];
        if line.len() == 0 {
            return self.remove_empty_line(self.cursor_pos.1);
        }
        match line.char_indices().nth(self.cursor_pos.0) {
            Some(result) => {
                line.remove(result.0);

                let value_to_sub = match self.mode == Mode::Insert {
                    //Insert mode can go a little bit out of the buffer
                    true => 0,
                    false => 1,
                };

                if line.len() > 0 && self.cursor_pos.0 > line.chars().count() - value_to_sub {
                    self.move_cursor_left(1);
                }
            }
            None => {
                log::warn!(
                    "Tried removing a character that is in a wrong index : {}",
                    self.cursor_pos.0
                );
            }
        }
        self.open_buffers[self.current_index].buffer.has_changes = true;
    }

    pub fn move_cursor_left(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        for _ in 0..amount {
            self.move_cursor_to(
                self.cursor_pos.0.checked_sub(1).unwrap_or(0),
                self.cursor_pos.1,
            );
        }
        self.latest_x = Some(self.cursor_pos.0);
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
    }

    pub fn move_cursor_right(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let value_to_sub = match self.mode == Mode::Insert {
            //Normal mode can go a little bit out of the buffer
            true => 0,
            false => 1,
        };
        let n = self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1]
            .chars()
            .count()
            .saturating_sub(value_to_sub);
        for _ in 0..amount {
            self.move_cursor_to(std::cmp::min(self.cursor_pos.0 + 1, n), self.cursor_pos.1);
        }
        self.latest_x = Some(self.cursor_pos.0);
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
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

    fn next_line_cursor_index(&mut self, x: usize, current_y: usize, previous_y: usize) -> usize {
        let normal_len = &self.open_buffers[self.current_index].buffer.lines[current_y]
            .chars()
            .count();
        let value_to_sub = match self.mode == Mode::Insert {
            //Insert mode can go a little bit out of the buffer
            true => 0,
            false => 1,
        };
        let cursor_x = Self::length_with_tabs_at(
            &self.open_buffers[self.current_index].buffer.lines[previous_y],
            x,
            TABSTOP,
        )
        .saturating_sub(1);

        // We need to find the shiftwidth on the cursor_x on the line below us so we can shift
        // accordingly, this is because a line under can have any arbitrary number of \t on any
        // arbitrary index, so finding the correct index to move to is crucial, it behaves both
        // differently to vscode and vim but its fine I think
        let mut shiftwidth = 0;
        let mut i = 0;
        for c in self.open_buffers[self.current_index].buffer.lines[current_y].chars() {
            if c == '\t' {
                let add = Self::get_spaces_till_next_tab(i + shiftwidth, TABSTOP).saturating_sub(1);
                shiftwidth += add;
            }
            if i + shiftwidth >= cursor_x || i > *normal_len {
                break;
            }
            i += 1;
        }
        std::cmp::min(normal_len.saturating_sub(value_to_sub), i)
    }

    pub fn move_cursor_down(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let previous_y = self.cursor_pos.1;
        self.move_cursor_to(
            self.cursor_pos.0,
            std::cmp::min(
                self.cursor_pos.1 + amount,
                self.open_buffers[self.current_index].buffer.lines.len() - 1,
            ),
        );
        if self.cursor_pos.1 != previous_y {
            // If we are not in the very last line
            if let Some(previous_x) = self.latest_x {
                let new_x = self.next_line_cursor_index(previous_x, self.cursor_pos.1, previous_y);
                self.move_cursor_to(std::cmp::min(previous_x, new_x), self.cursor_pos.1);
            }
        }
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
    }

    pub fn move_cursor_up(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let previous_y = self.cursor_pos.1;
        self.move_cursor_to(self.cursor_pos.0, self.cursor_pos.1.saturating_sub(amount));
        if previous_y != 0 {
            if let Some(previous_x) = self.latest_x {
                let new_x = self.next_line_cursor_index(previous_x, self.cursor_pos.1, previous_y);
                self.move_cursor_to(std::cmp::min(previous_x, new_x), self.cursor_pos.1);
            }
        }
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
    }

    fn join_lines(&mut self, first_line: usize, second_line: usize) {
        if first_line == second_line {
            return;
        };
        let mut first_string =
            self.open_buffers[self.current_index].buffer.lines[first_line].to_string();
        first_string
            .push_str(self.open_buffers[self.current_index].buffer.lines[second_line].as_str());

        self.open_buffers[self.current_index].buffer.lines[first_line] = first_string;
        self.open_buffers[self.current_index]
            .buffer
            .lines
            .remove(second_line);
        self.open_buffers[self.current_index].buffer.has_changes = true;
    }

    pub fn move_to(&mut self, c: char, amount: usize, offset: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let mut n = amount;
        let curr_line = &self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1];
        let mut skip_amount = 0;
        for (i, char) in curr_line.chars().skip(self.cursor_pos.0 + 1).enumerate() {
            if n == 0 {
                break;
            }
            if char == c {
                skip_amount = i + offset;
                n -= 1;
            }
        }
        self.move_cursor_to(
            std::cmp::min(self.cursor_pos.0 + skip_amount, curr_line.chars().count()),
            self.cursor_pos.1,
        );
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
    }

    pub fn move_word(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let mut loop_y = self.cursor_pos.1;
        let mut loop_x = self.cursor_pos.0;
        let mut result = MoveInfo {
            start_pos: self.cursor_pos,
            end_pos: self.cursor_pos,
        };
        let mut n = amount;
        // This would be more efficient if we didn't have a Vec<String> but whatever
        while loop_y < self.open_buffers[self.current_index].buffer.lines.len() {
            //Handle line_start
            if loop_x == 0
                && loop_y != self.cursor_pos.1
                && self.open_buffers[self.current_index].buffer.lines[loop_y]
                    .chars()
                    .next()
                    .map_or(false, |c| !is_seperator(c))
            {
                n -= 1;
                self.move_cursor_to(loop_x, loop_y);
                self.latest_x = Some(loop_x);
                result = MoveInfo {
                    start_pos: start,
                    end_pos: self.cursor_pos,
                };
                if n == 0 {
                    return result;
                }
            }
            let f = self.open_buffers[self.current_index].buffer.lines[loop_y]
                .chars()
                .skip(loop_x)
                .enumerate()
                .find(|c| is_seperator(c.1));
            if let Some(found) = f {
                //Found first but now we gotta keep consuming the whitespace
                let consumed = self.open_buffers[self.current_index].buffer.lines[loop_y]
                    .chars()
                    .skip(loop_x + found.0 + 1)
                    .take_while(|c| is_seperator(*c))
                    .count()
                    + 1;
                let to_skip = found.0 + consumed;
                n -= 1;
                self.move_cursor_to(loop_x + to_skip, loop_y);
                self.latest_x = Some(loop_x + to_skip);
                result = MoveInfo {
                    start_pos: start,
                    end_pos: self.cursor_pos,
                };
                if n == 0 {
                    return result;
                }
                loop_x += to_skip;
                continue;
            } else {
                if loop_y == self.open_buffers[self.current_index].buffer.lines.len() - 1 {
                    // meaning we are in the last line
                    let new_x = self.open_buffers[self.current_index].buffer.lines[self
                        .open_buffers[self.current_index].buffer
                        .lines
                        .len()
                        .saturating_sub(1)]
                    .chars()
                    .count()
                    .saturating_sub(1);

                    self.latest_x = Some(new_x);
                    self.move_cursor_to(new_x, loop_y);
                    return MoveInfo {
                        start_pos: start,
                        end_pos: self.cursor_pos,
                    };
                }
            }
            loop_y += 1;
            loop_x = 0;
        }
        result
    }

    pub fn move_end_word(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let mut loop_y = self.cursor_pos.1;
        let mut loop_x = self.cursor_pos.0 + 1;
        let mut result = MoveInfo {
            start_pos: self.cursor_pos,
            end_pos: self.cursor_pos,
        };
        let mut n = amount;
        // This would be more efficient if we didn't have a Vec<String> but whatever
        while loop_y < self.open_buffers[self.current_index].buffer.lines.len() {
            let f = self.open_buffers[self.current_index].buffer.lines[loop_y]
                .chars()
                .skip(loop_x)
                .enumerate()
                .find(|c| !is_seperator(c.1));
            if let Some(found) = f {
                //Found first but now we gotta keep consuming the whitespace
                let consumed = self.open_buffers[self.current_index].buffer.lines[loop_y]
                    .chars()
                    .skip(loop_x + found.0 + 1)
                    .take_while(|c| !is_seperator(*c))
                    .count();
                let to_skip = found.0 + consumed;
                n -= 1;
                self.move_cursor_to(loop_x + to_skip, loop_y);
                self.latest_x = Some(loop_x + to_skip);
                result = MoveInfo {
                    start_pos: start,
                    end_pos: self.cursor_pos,
                };
                if n == 0 {
                    return result;
                }
                loop_x += to_skip;
                continue;
            }
            loop_y += 1;
            loop_x = 0;
        }
        result
    }

    // These two functions are very similar but changing the functionality to go backwards makes
    // them unreadable messes
    pub fn move_end_word_backwards(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let mut loop_y = self.cursor_pos.1;
        let mut loop_x = self.cursor_pos.0;
        let mut result = MoveInfo {
            start_pos: self.cursor_pos,
            end_pos: self.cursor_pos,
        };
        let mut n = amount;
        // This would be more efficient if we didn't have a Vec<String> but whatever
        let mut len = self.open_buffers[self.current_index].buffer.lines[loop_y]
            .chars()
            .count();
        loop {
            let f = self.open_buffers[self.current_index].buffer.lines[loop_y]
                .chars()
                .rev()
                .skip(len.saturating_sub(loop_x))
                .enumerate()
                .find(|c| !is_seperator(c.1));
            if let Some(found) = f {
                //Found first but now we gotta keep consuming the whitespace
                let consumed = self.open_buffers[self.current_index].buffer.lines[loop_y]
                    .chars()
                    .rev()
                    .skip(len.saturating_sub(loop_x + found.0))
                    .take_while(|c| !is_seperator(*c))
                    .count();
                let to_skip = found.0 + consumed;
                n -= 1;
                self.move_cursor_to(loop_x.saturating_sub(to_skip), loop_y);
                result = MoveInfo {
                    start_pos: start,
                    end_pos: self.cursor_pos,
                };
                if n == 0 {
                    return result;
                }
                loop_x = loop_x.saturating_sub(to_skip);
                continue;
            } else {
                //Handle case for when we are going back to the beggining of a line, if we didn't
                //start at the begginng then we want to go there before skipping
                if loop_x > 0 {
                    n -= 1;
                    self.move_cursor_to(0, loop_y);
                    result = MoveInfo {
                        start_pos: start,
                        end_pos: self.cursor_pos,
                    };
                    if n == 0 {
                        return result;
                    }
                }
            }
            if let Some(y) = loop_y.checked_sub(1) {
                loop_y = y;
                len = self.open_buffers[self.current_index].buffer.lines[y]
                    .chars()
                    .count(); // This ensures we skip nothing when len - loop_x is done, its a hack
                loop_x = len;
            } else {
                break;
            }
        }
        result
    }

    // Theres alot of edge cases
    pub fn delete_selection(&mut self, movement: MoveInfo) {
        let m = movement.get_ordered();
        let (start_x, start_y) = m.start_pos;
        let (end_x, end_y) = m.end_pos;

        // We just delete from start_x to end_x if it doesn't span any lines
        if start_y == end_y {
            let mut s = String::new();
            for (i, c) in self.open_buffers[self.current_index].buffer.lines[start_y]
                .chars()
                .enumerate()
            {
                if !(i >= start_x && i <= end_x) {
                    s.push(c);
                }
            }
            if s.len() == 0 {
                self.open_buffers[self.current_index]
                    .buffer
                    .lines
                    .remove(start_y);
                // self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(1);
                self.move_cursor_to(self.cursor_pos.0, self.cursor_pos.1.saturating_sub(1));
            } else {
                self.open_buffers[self.current_index].buffer.lines[start_y] = s;
            }
            return;
        }

        //Another edge case, if we are deleting something that spans a single word over line
        //boundaries then we delete only that one word until the line end

        //So we can remove everything at once
        let mut remove_indices = Vec::new();
        //We gotta delete the beggining
        {
            let mut s = String::new();
            for (i, c) in self.open_buffers[self.current_index].buffer.lines[start_y]
                .chars()
                .enumerate()
            {
                if !(i >= start_x) {
                    s.push(c);
                }
            }
            // if s.len() == 0 {
            //     remove_indices.push(start_y);
            // }
            // else{
            self.open_buffers[self.current_index].buffer.lines[start_y] = s;
            // }
        }

        {
            let mut s = String::new();
            for (i, c) in self.open_buffers[self.current_index].buffer.lines[end_y]
                .chars()
                .enumerate()
            {
                if !(i <= end_x) {
                    s.push(c);
                }
            }
            // if s.len() == 0 {
            //     remove_indices.push(end_y);
            // }
            // else{
            self.open_buffers[self.current_index].buffer.lines[end_y] = s;
            // }
        }

        let lines_between = end_y.saturating_sub(start_y).saturating_sub(1);
        for i in 0..lines_between {
            remove_indices.push(start_y + i + 1);
        }

        //We sort so we can remove from bottom to top therefore preserving our indices
        remove_indices.sort_by(|a, b| b.cmp(a));
        // self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(remove_indices.len());
        self.move_cursor_to(
            self.cursor_pos.0,
            self.cursor_pos.1.saturating_sub(remove_indices.len()),
        );
        for i in remove_indices {
            self.open_buffers[self.current_index].buffer.lines.remove(i);
        }

        //now we gotta join the start and end lines
        let last_line = self.open_buffers[self.current_index].buffer.lines[start_y + 1].clone();
        self.open_buffers[self.current_index].buffer.lines[start_y].push_str(&last_line);
        self.open_buffers[self.current_index]
            .buffer
            .lines
            .remove(start_y + 1);

        if self.open_buffers[self.current_index].buffer.lines[start_y].len() == 0
            && self.open_buffers[self.current_index].buffer.lines.len() > 1
        {
            self.open_buffers[self.current_index]
                .buffer
                .lines
                .remove(start_y);
        }
    }

    pub fn move_cursor_to(&mut self, x: usize, y: usize) {
        // let to_sub = if matches!(self.mode, Mode::Insert) { 0} else {1};
        // self.cursor_pos.1 = std::cmp::min(y, self.open_buffers[self.current_index].buffer.lines.len().saturating_sub(1));
        // self.cursor_pos.0  = std::cmp::min(x, self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1].chars().count().saturating_sub(to_sub));
        self.cursor_pos.0 = x;
        // self.cursor_pos.1 = y;
        self.cursor_pos.1 = std::cmp::min(y, self.curr_buffer().lines.len().saturating_sub(1));
        if self.mode == Mode::Visual {
            if let Some(select) = &self.curr_selection {
                log::info!(
                    "x {} y {} selection {:?}",
                    self.cursor_pos.0,
                    self.cursor_pos.1,
                    select
                );

                self.curr_selection = Some((
                    select.0,
                    MoveInfo {
                        start_pos: select.0,
                        end_pos: self.cursor_pos,
                    }
                    .get_ordered(),
                ));
                // Some(select.expand_or_shrink(self.cursor_pos.0, self.cursor_pos.1));
                log::info!("new selection {:?}", self.curr_selection);
            }
        }
        self.open_buffers[self.current_index].cursor_pos = self.cursor_pos;
    }

    pub fn character_at_cursor(&self) -> char {
        self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1]
            .chars()
            .skip(self.cursor_pos.0)
            .next()
            .unwrap_or(' ')
    }

    pub fn delete_lines(&mut self, movement: MoveInfo) {
        let m = movement.get_ordered();
        let (_, start_y) = m.start_pos;

        let num_lines = m.end_pos.1.saturating_sub(start_y) + 1;
        for _ in 0..num_lines {
            if self.open_buffers[self.current_index].buffer.lines.len() == 1 {
                // We have deleted essentially everything
                self.open_buffers[self.current_index].buffer.lines[0] = String::new();
                break;
            }
            self.open_buffers[self.current_index]
                .buffer
                .lines
                .remove(start_y);
        }
    }

    pub fn switch_mode(&mut self, new_mode: Mode) {
        match new_mode {
            Mode::Normal => {
                if self.mode == Mode::Insert {
                    self.move_cursor_left(1);
                    self.move_cursor_left(1);
                    self.mode = Mode::Normal;
                    self.move_cursor_right(1);
                } else {
                    self.mode = Mode::Normal;
                }
                self.curr_selection = None;
            }
            Mode::Insert => {
                self.mode = Mode::Insert;
                self.curr_selection = None;
            }
            Mode::Visual => {
                self.mode = Mode::Visual;
                self.curr_selection = Some((
                    self.cursor_pos,
                    MoveInfo {
                        start_pos: self.cursor_pos,
                        end_pos: self.cursor_pos,
                    },
                ))
            }
        }
    }

    pub fn move_to_end(&mut self) -> MoveInfo {
        let start_pos = self.cursor_pos;
        let new_x = self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1]
            .chars()
            .count()
            - 1;
        self.move_cursor_to(new_x, self.cursor_pos.1);
        self.latest_x = Some(new_x);
        MoveInfo {
            start_pos,
            end_pos: self.cursor_pos,
        }
    }

    pub fn copy(&mut self, selection: MoveInfo) -> MoveInfo {
        let mut result = Vec::new();
        let (start_x, start_y) = selection.start_pos;
        let (end_x, end_y) = selection.end_pos;
        let mut m = MoveInfo {
            start_pos: selection.start_pos.clone(),
            end_pos: selection.end_pos.clone(),
        };

        let take_amount = if start_y == end_y {
            end_x - start_x
        } else {
            self.open_buffers[self.current_index].buffer.lines[start_y]
                .chars()
                .count()
                - start_x
        };
        let content = self.open_buffers[self.current_index].buffer.lines[start_y]
            .chars()
            .skip(start_x)
            .take(take_amount)
            .collect::<String>();
        result.push(content.as_str());

        let num_lines = end_y.saturating_sub(start_y);

        for i in 1..num_lines {
            log::info!("What");
            result.push(self.open_buffers[self.current_index].buffer.lines[start_y + i].as_str());
        }

        //if our start_y and end_y are differents we need to take the remainder of the string as
        //well
        let remainder;
        if start_y != end_y {
            let len = self.open_buffers[self.current_index].buffer.lines[end_y]
                .chars()
                .count();
            let take_amount = if end_x == len - 1 { len } else { end_x };
            m.end_pos.1 = take_amount.clone();
            remainder = self.open_buffers[self.current_index].buffer.lines[end_y]
                .chars()
                .take(take_amount)
                .collect::<String>();
            result.push(remainder.as_str());
        }
        self.clipboard.set_contents(result.join("\n")).unwrap();
        log::info!("{}", self.clipboard.get_contents().unwrap());
        m
    }
    pub fn copy_lines(&mut self, movement: MoveInfo) -> MoveInfo {
        let m = movement.get_ordered();
        let (_, start_y) = m.start_pos;

        let num_lines = m.end_pos.1.saturating_sub(start_y) + 1;
        let mut contents = Vec::new();
        for i in 0..num_lines {
            contents.push(self.open_buffers[self.current_index].buffer.lines[start_y + i].as_str());
        }

        let mut clipboard_contents = contents.join("\n");
        clipboard_contents.push('\n');
        self.clipboard.set_contents(clipboard_contents).unwrap();
        MoveInfo {
            start_pos: (0, m.start_pos.1),
            end_pos: (
                self.open_buffers[self.current_index].buffer.lines
                    [start_y + num_lines.saturating_sub(1)]
                .chars()
                .count(),
                start_y + num_lines.saturating_sub(1),
            ),
        }
    }

    fn paste_lines(&mut self) {
        let split: Vec<String> = self
            .clipboard
            .get_contents()
            .unwrap()
            .split('\n')
            .map(|s| s.to_string())
            .collect();

        let mut new_lines = Vec::new();
        for (i, str) in self.open_buffers[self.current_index]
            .buffer
            .lines
            .iter()
            .enumerate()
        {
            if i == self.cursor_pos.1 {
                new_lines.push(str.to_string());
                for s in split.as_slice().iter().take(split.len() - 1) {
                    new_lines.push(s.to_string());
                }
            } else {
                new_lines.push(str.to_string());
            }
        }
        self.open_buffers[self.current_index].buffer.lines = new_lines;
        self.cursor_pos.1 += 1;
    }

    pub fn paste(&mut self) {
        // paste is a bit more complicated than this
        let binding = self.clipboard.get_contents().unwrap();
        log::info!("{}", binding);
        // figure out where or not the content we have are full lines
        if let Some(c) = binding.chars().rev().next() {
            if c == '\n' {
                return self.paste_lines();
            }
        }

        let binding_len = binding.chars().count();
        let mut copy =
            self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1].clone();
        copy.insert_str(
            std::cmp::min(self.cursor_pos.0 + 1, copy.chars().count()),
            &binding,
        );

        let split = copy.split('\n');

        let len = self.open_buffers[self.current_index].buffer.lines.len();
        let mut save = 0;
        for (i, str) in split.enumerate() {
            if i == 0 {
                self.open_buffers[self.current_index].buffer.lines[self.cursor_pos.1 + i] =
                    str.to_string();
            } else if self.cursor_pos.1 + i < len {
                self.open_buffers[self.current_index]
                    .buffer
                    .lines
                    .insert(self.cursor_pos.1 + i, str.to_string());
            } else {
                self.open_buffers[self.current_index]
                    .buffer
                    .lines
                    .push(str.to_string());
            }
            save = i;
        }

        // Have cursor follow
        if save == 0 {
            self.move_cursor_to(self.cursor_pos.0 + binding_len, self.cursor_pos.1);
        }
    }

    pub fn close_current_buffer(&mut self) {
        let curr_buffer = self.curr_buffer();
        if self.curr_buffer().has_changes {
            let name = match &curr_buffer.path {
                Some(name) => name.to_string(),
                None => "Unnamed".to_string(),
            };
            self.message = format!("File \"{}\" has unwritten changes", name);
            return;
        }

        self.open_buffers.remove(self.current_index);
        self.current_index = self.open_buffers.len().saturating_sub(1);

        //if we close the last buffer we should open an unamed one
        if self.open_buffers.len() == 0 {
            self.open_buffers.push(TextBufferInfo{buffer:TextBuffer::empty(), cursor_pos: (0,0)});
        }
        self.cursor_pos = self.open_buffers[self.current_index].cursor_pos;
    }

    pub fn next_buffer(&mut self){
        self.current_index = (self.current_index + 1) % self.open_buffers.len();
        self.cursor_pos = self.open_buffers[self.current_index].cursor_pos;
    }

    pub fn prev_buffer(&mut self){
        let current_index = self.current_index as i64;
        let len = self.open_buffers.len() as i64;
        self.current_index = ((current_index  - 1) % len )as usize;
        self.cursor_pos = self.open_buffers[self.current_index].cursor_pos;
    }
}
