pub mod buffer;

use crate::editor::buffer::TextBuffer;

const DEFAULT_FILE_PATH: &str = "default.txt";
pub(crate) const TABSTOP: usize = 2;

pub fn is_seperator(c: char) -> bool {
    !c.is_alphanumeric() || c.is_whitespace()
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoveInfo {
    pub start_pos: (usize, usize),
    pub end_pos: (usize, usize),
}

impl MoveInfo {
    pub fn is_backwards(&self) -> bool {
        self.start_pos.1 >= self.end_pos.1 && self.start_pos.0 > self.end_pos.0
    }

    // as in, start pos is before end pos
    pub fn get_ordered(&self) -> Self{
        if self.is_backwards(){
            MoveInfo{
                start_pos: self.end_pos,
                end_pos : self.start_pos,
            }
        }
        else{
            self.clone()
        }
    }
}

#[derive(PartialEq, Clone)]
#[derive(Debug)]
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
        self.move_cursor_left(1);
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
        self.move_cursor_up(1);
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
                    // self.move_cursor_left()
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

    pub fn move_cursor_left(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        for _ in 0..amount {
            self.cursor_pos.0 = self.cursor_pos.0.checked_sub(1).unwrap_or(0);
        }
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
    }

    pub fn move_cursor_right(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let value_to_sub = match self.mode == Mode::Normal {
            //Normal mode can go a little bit out of the buffer
            true => 1,
            false => 0,
        };
        let n = self.buffer.lines[self.cursor_pos.1]
            .chars()
            .count()
            .saturating_sub(value_to_sub);
        for _ in 0..amount {
            self.cursor_pos.0 = std::cmp::min(self.cursor_pos.0 + 1, n);
        }
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

    fn next_line_cursor_index(&mut self, current_y: usize, previous_y: usize) -> usize {
        let normal_len = &self.buffer.lines[current_y].chars().count();
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
        self.cursor_pos.1 = std::cmp::min(self.cursor_pos.1 + amount, self.buffer.lines.len() - 1);
        if self.cursor_pos.1 != self.buffer.lines.len() {
            // If we are not in the very last line
            self.cursor_pos.0 = self.next_line_cursor_index(self.cursor_pos.1, previous_y);
        }
        MoveInfo {
            start_pos: start,
            end_pos: self.cursor_pos,
        }
    }

    pub fn move_cursor_up(&mut self, amount: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let previous_y = self.cursor_pos.1;
        self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(amount);
        if previous_y != 0 {
            self.cursor_pos.0 = self.next_line_cursor_index(self.cursor_pos.1, previous_y);
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
        let mut first_string = self.buffer.lines[first_line].to_string();
        first_string.push_str(self.buffer.lines[second_line].as_str());

        self.buffer.lines[first_line] = first_string;
        self.buffer.lines.remove(second_line);
        self.buffer.has_changes = true;
    }

    pub fn move_to(&mut self, c: char, amount: usize, offset: usize) -> MoveInfo {
        let start = self.cursor_pos;
        let mut n = amount;
        let curr_line = &self.buffer.lines[self.cursor_pos.1];
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
        self.cursor_pos.0 =
            std::cmp::min(self.cursor_pos.0 + skip_amount, curr_line.chars().count());
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
        while loop_y < self.buffer.lines.len() {

            //Handle line_start
            if loop_x == 0 && loop_y != self.cursor_pos.1 && self.buffer.lines[loop_y].chars().next().map_or(false, |c| !is_seperator(c)){
                n -= 1;
                self.cursor_pos.1 = loop_y;
                self.cursor_pos.0 = loop_x;
                result = MoveInfo {
                    start_pos: start,
                    end_pos: self.cursor_pos,
                };
                if n == 0 {
                    return result;
                }

            }
            let f = self.buffer.lines[loop_y]
                .chars()
                .skip(loop_x)
                .enumerate()
                .find(|c| is_seperator(c.1));
            if let Some(found) = f {
                //Found first but now we gotta keep consuming the whitespace
                let consumed = self.buffer.lines[loop_y]
                    .chars()
                    .skip(loop_x + found.0 + 1)
                    .take_while(|c| is_seperator(*c))
                    .count()
                    ;
                let to_skip = found.0 + consumed;
                n -= 1;
                self.cursor_pos.1 = loop_y;
                self.cursor_pos.0 = loop_x + to_skip;
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
            else {
                if loop_y == self.buffer.lines.len() -1{ // meaning we are in the last line
                    self.cursor_pos.0 = self.buffer.lines[self.buffer.lines.len().saturating_sub(1)].chars().count().saturating_sub(1);
                    self.cursor_pos.1 = loop_y;
                    return MoveInfo{
                        start_pos : start,
                        end_pos : self.cursor_pos,
                    }
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
        while loop_y < self.buffer.lines.len() {

            let f = self.buffer.lines[loop_y]
                .chars()
                .skip(loop_x)
                .enumerate()
                .find(|c| !is_seperator(c.1));
            if let Some(found) = f {
                //Found first but now we gotta keep consuming the whitespace
                let consumed = self.buffer.lines[loop_y]
                    .chars()
                    .skip(loop_x + found.0 + 1)
                    .take_while(|c| !is_seperator(*c))
                    .count();
                let to_skip = found.0 + consumed;
                n -= 1;
                self.cursor_pos.1 = loop_y;
                self.cursor_pos.0 = loop_x + to_skip;
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
        let mut len = self.buffer.lines[loop_y].chars().count();
        loop {
            let f = self.buffer.lines[loop_y]
                .chars()
                .rev()
                .skip(len.saturating_sub(loop_x))
                .enumerate()
                .find(|c| !is_seperator(c.1));
            if let Some(found) = f {
                //Found first but now we gotta keep consuming the whitespace
                let consumed = self.buffer.lines[loop_y]
                    .chars()
                    .rev()
                    .skip(len.saturating_sub(loop_x + found.0))
                    .take_while(|c| !is_seperator(*c))
                    .count();
                let to_skip = found.0 + consumed;
                n -= 1;
                self.cursor_pos.1 = loop_y;
                self.cursor_pos.0 = loop_x.saturating_sub(to_skip);
                result = MoveInfo {
                    start_pos: start,
                    end_pos: self.cursor_pos,
                };
                if n == 0 {
                    return result;
                }
                loop_x = loop_x.saturating_sub(to_skip);
                continue;
            }
            else {
                //Handle case for when we are going back to the beggining of a line, if we didn't
                //start at the begginng then we want to go there before skipping 
                if loop_x > 0 {
                    n -= 1;
                    self.cursor_pos.1 = loop_y;
                    self.cursor_pos.0 = 0;
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
                len = self.buffer.lines[y].chars().count(); // This ensures we skip nothing when len - loop_x is done, its a hack
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
            for (i,c) in self.buffer.lines[start_y].chars().enumerate(){
                if !(i >= start_x && i <= end_x){
                   s.push(c); 
                }
            }
            if s.len() == 0 {
                self.buffer.lines.remove(start_y);
                self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(1);
            }
            else{
                self.buffer.lines[start_y] = s;
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
            for (i,c) in self.buffer.lines[start_y].chars().enumerate(){
                if !(i >= start_x){
                   s.push(c); 
                }
            }
            // if s.len() == 0 {
            //     remove_indices.push(start_y);
            // }
            // else{
                self.buffer.lines[start_y] = s;
            // }
        }

        {
            let mut s = String::new();
            for (i,c) in self.buffer.lines[end_y].chars().enumerate(){
                if !(i <= end_x){
                   s.push(c); 
                }
            }
            // if s.len() == 0 {
            //     remove_indices.push(end_y);
            // }
            // else{
                self.buffer.lines[end_y] = s;
            // }
        }

        let lines_between = end_y.saturating_sub(start_y).saturating_sub(1);
        for i in 0..lines_between {
           remove_indices.push(start_y + i + 1);
        }

        //We sort so we can remove from bottom to top therefore preserving our indices
        remove_indices.sort_by(|a,b| b.cmp(a));
        self.cursor_pos.1 = self.cursor_pos.1.saturating_sub(remove_indices.len());
        for i in remove_indices {
           self.buffer.lines.remove(i); 
        }

        //now we gotta join the start and end lines
        let last_line = self.buffer.lines[start_y + 1].clone();
        self.buffer.lines[start_y].push_str(&last_line);
        self.buffer.lines.remove(start_y + 1);

        if self.buffer.lines[start_y].len() == 0 && self.buffer.lines.len() > 1 {
            self.buffer.lines.remove(start_y);
        }
    }

    pub fn move_cursor_to(&mut self, x : usize, y : usize){
        self.cursor_pos.1 = std::cmp::min(y, self.buffer.lines.len().saturating_sub(1));
        self.cursor_pos.0  = std::cmp::min(x, self.buffer.lines[self.cursor_pos.1].chars().count().saturating_sub(1));
    }

    pub fn character_at_cursor(&self) -> char{
        self.buffer.lines[self.cursor_pos.1].chars().skip(self.cursor_pos.0).next().unwrap_or(' ')
    }

    pub fn delete_lines(&mut self, movement: MoveInfo) {
        let m = movement.get_ordered();
        let (_, start_y ) = m.start_pos;

        let num_lines = m.end_pos.1.saturating_sub(start_y) + 1;
        for _ in 0..num_lines{
            if self.buffer.lines.len() == 1 {
                // We have deleted essentially everything
                self.buffer.lines[0] = String::new();
                break;
            }
            self.buffer.lines.remove(start_y);
        }
    }
}
