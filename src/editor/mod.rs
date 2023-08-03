pub mod line;

use crate::editor::line::Line;

pub struct Editor{
    buffer : String,
    pub cursor_index : usize,
    pub lines : Vec<Line>,
}

impl Editor{
    pub fn new() -> Editor{
        let initial_text = "Hello my name is\nSomething";
        Editor{
            lines : Vec::new(),
            cursor_index : 0,
            buffer : String::from(initial_text),
        }
    }

    pub fn compute_lines(&mut self) -> Vec<Line>{
       Line::compute_lines(&self.buffer)
    }

    pub fn get_buffer(&self) -> String{
        self.buffer.to_owned()
    }

    pub fn pop_char(&mut self) -> Option<char>{
        if self.cursor_index == 0 {
            None
        }
        else{
            self.cursor_index -= 1;
            let (index , _char) = self.buffer.char_indices().nth(self.cursor_index).expect("Somethign went terribly wrong with removing");
            Some(self.buffer.remove(index))
        }
    }

    pub fn push_char(&mut self, c : char){
        let (index , _char) = self.buffer.char_indices().nth(self.cursor_index).expect("Somethign went terribly wrong with putting");
        self.buffer.insert(index, c);
        self.cursor_index += 1;
    }

}

