pub mod line;

use crate::editor::line::Line;
use std::fs;

pub struct Editor{
    buffer : String,
    buffer_path : String,
    pub cursor_index : usize,
    pub lines : Vec<Line>,
}

impl Editor{
    pub fn new() -> Editor{
        let _initial_text = "Hello my name is\nSomething\n";
        Editor{
            lines :Vec::new(),
            cursor_index : 0,
            buffer : String::from(_initial_text),
            buffer_path : String::new(),
        }
    }

    pub fn open_file(&mut self, path: &str){
        let buff = fs::read_to_string(path).unwrap();
        self.buffer_path = String::from(path);
        self.buffer = buff;
    }

    pub fn get_cursor_line(&self) -> Option<(usize, &Line)>{
        self.lines.iter().enumerate().find(|line| self.cursor_index >=line.1.start && self.cursor_index <= line.1.end)
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
        match self.buffer.char_indices().nth(self.cursor_index){
            Some(result) =>{
                self.buffer.insert(result.0, c);
                self.cursor_index += 1;
            }
            None =>{
                self.buffer.push(c);
                self.cursor_index += 1;
            }
        }
        // let (index , _char) = self.buffer.char_indices().nth(self.cursor_index);
        // self.buffer.insert(index, c);
        // self.cursor_index += 1;
    }

    pub fn move_cursor_right(&mut self){
        self.cursor_index = std::cmp::min(self.cursor_index + 1,self.buffer.len());
    }

    pub fn move_cursor_left(&mut self){
        if self.cursor_index != 0 {
            self.cursor_index -= 1;
        }
    }

    pub fn move_cursor_up(&mut self){
        let (curr_index, curr_line) = self.get_cursor_line().unwrap();
        if curr_index > 0{
            let above = &self.lines[curr_index - 1];
            let index_within = self.cursor_index - curr_line.start;
            if above.size <= index_within{
                self.cursor_index = above.end;
            }
            else{
                self.cursor_index = above.start + index_within;
            }
        }
    }

    pub fn move_cursor_down(&mut self){
        let (curr_index, curr_line) = self.get_cursor_line().unwrap();
        if curr_index + 1 < self.lines.len(){
            let below = &self.lines[curr_index +1];
            let index_within = self.cursor_index - curr_line.start;
            if below.size <= index_within{
                self.cursor_index = below.end;
            }
            else{
                self.cursor_index = below.start + index_within;
            }
        }
    }

    pub fn save_file(&self){
        fs::write(&self.buffer_path, &self.buffer).unwrap();
    }

}

