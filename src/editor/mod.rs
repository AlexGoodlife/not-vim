pub mod line;

use crate::editor::line::Line;
use std::fs;

pub struct Editor{
    buffer : String,
    buffer_path : String,
    pub cursor_index : usize,
    pub lines : Vec<Line>,
    write_info : (bool,usize, usize),
}

impl Editor{
    pub fn new() -> Editor{
        let _initial_text = "Hello my name is\nSomething\n";
        Editor{
            lines :Vec::new(),
            cursor_index : 0,
            buffer : String::from(_initial_text),
            buffer_path : String::new(),
            write_info : (false,_initial_text.len(), 3),
        }
    }

    pub fn open_file(&mut self, path: &str){
        let buff = fs::read_to_string(path).unwrap();
        self.buffer_path = String::from(path);
        self.buffer = buff;
        self.compute_lines(0, std::usize::MAX);
        self.write_info = (false, self.buffer.len(),self.lines.len());
    }

    pub fn get_cursor_line(&self) -> Option<(usize, &Line)>{
        self.lines.iter().enumerate().find(|line| self.cursor_index >=line.1.start && self.cursor_index <= line.1.end)
    }

    pub fn compute_lines(&mut self, offset: usize, max_lines : usize){
        let mut result = Vec::new();
        let values = self.buffer.chars();
        let mut start = 0;
        let mut end = 0;
        for c in values{
            if c == '\n'{
                result.push(Line::new(start,end));
                start = end+1;
                end = start;
                continue;
            }
            end += 1;
        }
        result.push(Line::new(start,end));
        self.lines = result;
    }

    // pub fn compute_lines(&mut self, offset : usize, max_lines : usize){
    //     // let mut result = Vec::new();
    //     // let current_line = self.get_cursor_line();
    //     let mut skip = 0;
    //     let mut idx = 0;
    //     if let Some(current_line) = self.get_cursor_line(){
    //         skip  = current_line.1.start;
    //         idx = current_line.0;
    //     }
    //
    //     let values = self.buffer.chars().skip(skip);
    //     let limit = max_lines + offset;
    //     let mut start = skip;
    //     let mut end = skip;
    //     for c in values{
    //         if idx >= limit{
    //             break;
    //         }
    //         if c == '\n'{
    //             if idx >= self.lines.len(){
    //                 self.lines.push(Line::new(start, end));
    //             }
    //             else{
    //                 let _ = std::mem::replace(&mut self.lines[idx], Line::new(start,end));
    //                 // self.lines.shrink_to_fit(idx,Line::new(start, end));
    //             }
    //             start = end+1;
    //             end = start;
    //             idx +=1;
    //             continue;
    //         }
    //         end += 1;
    //     }
    //     if idx < limit{
    //         if idx >= self.lines.len(){
    //             self.lines.push(Line::new(start, end));
    //         }
    //         // self.lines.insert(idx, Line::new(start,end));
    //         // result.push(Line::new(start,end));
    //     }
    //     // self.lines = result;
    // }
    // pub fn compute_lines(&mut self) -> Vec<Line>{
    //    Line::compute_lines(&self.buffer)
    // }

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
        self.cursor_index = std::cmp::min(self.cursor_index + 1,self.buffer.len()-1);
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

    pub fn save_file(&mut self){
        fs::write(&self.buffer_path, &self.buffer).unwrap();
        self.write_info = (true, self.buffer.len(),self.lines.len());
    }

    pub fn get_status(&self) -> String{
        if !self.write_info.0{
            return format!("\"{}\" {}L {}B", &self.buffer_path, &self.write_info.2,&self.write_info.1);
        }
        else{
            return format!("saved \"{}\" {}L {}B written", &self.buffer_path,  &self.write_info.2,&self.write_info.1);
        }
    }

}

