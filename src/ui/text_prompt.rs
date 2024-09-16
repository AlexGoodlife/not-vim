//This models any basic text box with a singular string, we need to keep track of the cursor position and

pub struct TextPrompt {
    content: String,
    cursor_pos: usize,
}

impl TextPrompt {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn push_char(&mut self, c: char) {
        match self.content.char_indices().nth(self.cursor_pos) {
            Some(result) => {
                self.content.insert(result.0, c);
            }
            None => {
                self.content.push(c);
            }
        }
        self.cursor_pos += 1;
    }

    pub fn pop_char(&mut self) {
        match self
            .content
            .char_indices()
            .nth(self.cursor_pos.saturating_sub(1))
        {
            Some(result) => {
                self.content.remove(result.0);

                self.move_cursor_left(1);
            }
            None => {
                log::warn!(
                    "Tried removing a character that is in a wrong index : {}",
                    self.cursor_pos
                );
            }
        }
    }

    pub fn move_cursor_left(&mut self, amount: usize) {
        self.cursor_pos = self.cursor_pos.saturating_sub(amount);
    }

    pub fn move_cursor_right(&mut self, amount: usize) {
        self.cursor_pos = std::cmp::min(self.cursor_pos + amount, self.content.chars().count());
    }

    pub fn get_cursor(&self) -> usize {
        self.cursor_pos
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }
}
