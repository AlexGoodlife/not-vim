use event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use std::io::{Stdout, stdout, Write};

use event::{read, poll,Event};
use crossterm::{ execute, terminal, terminal::enable_raw_mode, terminal::disable_raw_mode,Result, style,event, queue, cursor,terminal::size
};
use crate::editor::Editor;
use crate::editor::line::Line;

pub struct EditorFlags{
    quit : bool,
    recompute : bool,
}

impl EditorFlags{
    pub fn new() -> EditorFlags{
        EditorFlags{
            quit : false,
            recompute : true,
        }
    }
}

struct Cursor{
    pub x : u16,
    pub y : u16,
}

impl Cursor{
    pub fn move_to_index(&mut self, lines: &Vec<Line>, index: usize, window_x : usize, window_y : usize, offset: &mut usize){
        let default_line = Line::new(0,0);
        let current_line = lines.iter().enumerate().find(|line| index >=line.1.start && index <= line.1.end).unwrap_or((0, &default_line));
        let row = current_line.0;
        let mut collumn =  index - current_line.1.start ;

        *offset = std::cmp::min(*offset,row);
        if row >= *offset + window_y{
            *offset = row as usize - window_y as usize + 1;
        }
        //For now we offset collumns by 2
        collumn += 2;
        self.x = collumn as u16;
        // self.y = 0 as u16;
        self.y = (row as usize - *offset as usize) as u16;
    }
    
}

pub struct Client{
    content : String,
    cursor : Cursor,
    editor : Editor,
    flags : EditorFlags,
    output : Stdout,
    window_dimensions : (u16,u16),
    window_offset : usize,
}

impl Drop for Client{
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.output, terminal::Clear(terminal::ClearType::All)).unwrap();
        println!("{:?}", self.window_offset);
        // println!("{:?}", self.editor.lines.len());
        // self.editor.lines.iter().enumerate().for_each(|(i,line)| {
        //     print!("{} ", i);
        //     println!("{:?}", line);
        // }
        // );
        // execute!(self.output, terminal::LeaveAlternateScreen).unwrap();
    }

}

impl Client{

    pub fn new() -> Self{
        Self{
            content : String::new(),
            cursor : Cursor{x : 0, y : 0},
            editor : Editor::new(),
            flags : EditorFlags::new(),
            output : stdout(),
            window_dimensions : size().unwrap_or((0,0)),
            window_offset : 0,
        }
    }

    fn should_quit(&self) -> bool{
        self.flags.quit
    }

    fn should_recompute(&self) -> bool{
        self.flags.recompute
    }

    // fn get_content(&mut self) -> String{
    //     let mut result = String::new();
    //     let mut i = 1;
    //     let buffer = self.editor.get_buffer();
    //     for line in &self.editor.lines{
    //         result.push_str(&i.to_string());
    //         result.push(' ');
    //         let to_append = buffer.chars().skip(line.start).take(line.size).collect::<String>();
    //         // println!("{:?}", to_append);
    //         result.push_str(&to_append);
    //         result.push('\r');
    //         i += 1;
    //     }
    //     result
    // }

    fn move_cursor(&self) -> Result<()>{
        queue!(&self.output, cursor::MoveTo(self.cursor.x, self.cursor.y))?;
        Ok(())
    }

    pub fn get_content(&mut self) {
        self.content.clear();
        let result = &mut self.content;
        let buffer = self.editor.get_buffer();

        //old version, dont take out
        // let rows = std::cmp::min(self.window_dimensions.1 as usize, self.editor.lines.len()-self.window_offset);
        
        let rows = self.window_dimensions.1 as usize;
        for i in 0..rows{
            let idx = i + self.window_offset;
            if idx >= self.editor.lines.len(){
                if i < rows-1{
                    result.push_str("~\r\n");
                }
                else{
                    result.push('~');
                }
                continue;
            }
            result.push_str(&(idx+1).to_string());
            result.push(' ');
            let mut to_append = buffer.chars().skip(self.editor.lines[idx].start).take(self.editor.lines[idx].size).collect::<String>();
            // weird stuff
            if let Some(character) = to_append.pop(){
                if character != '\n'{
                    to_append.push(character);
                }
                if i < rows-1 {
                    to_append.push('\r');
                    to_append.push('\n');
                }
            }
            result.push_str(&to_append);
        }
    }

    fn update(&mut self) -> Result<()>{
        let (window_x, window_y) = size().unwrap_or((0,0));
        self.cursor.move_to_index(&self.editor.lines, self.editor.cursor_index,window_x.into(), window_y.into(), &mut self.window_offset);
        self.move_cursor()?;
        if self.should_recompute() {
            self.editor.lines = self.editor.compute_lines();

            //this needs to be better implemented later
            self.cursor.move_to_index(&self.editor.lines, self.editor.cursor_index,window_x.into(), window_y.into(), &mut self.window_offset);
            self.get_content();
            queue!(self.output, terminal::Clear(terminal::ClearType::All))?;
            // queue!(self.output, cursor::SavePosition)?;
            queue!(self.output, cursor::MoveTo(0,0))?;
            queue!(self.output, cursor::Hide)?;

            queue!(self.output, style::Print(&self.content))?;

            // queue!(self.output, cursor::RestorePosition)?;
            queue!(self.output, crossterm::cursor::Show)?;
            self.flags.recompute = false;
            self.output.flush()?;
        }
        Ok(())
    }

    fn handle_keys(&mut self, ev : event::KeyEvent) -> Result<()>{
        match ev{
            KeyEvent{
                code : KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
            } => {self.flags.quit = true;},
            KeyEvent{
                code: KeyCode::Char(character),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
            } => {
                    self.editor.push_char(character);
                    self.flags.recompute = true;
                }
            KeyEvent{
                code : KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
            } => {
                    self.editor.pop_char();
                    self.flags.recompute = true;
                }
            KeyEvent{
                code : KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            } => {
                    self.editor.push_char('\n');
                    self.flags.recompute = true;
                }
            _ => println!("{:?}", ev.code)
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()>{
        if poll(Duration::from_secs(0))? {
            match read()?{
                Event::Key(ev) => self.handle_keys(ev)?,
                _ => println!("Some other event"),
            }

        }
        Ok(())
    }
    pub fn run(&mut self) -> Result<()>{
        // execute!(self.output, terminal::EnterAlternateScreen)?;
        enable_raw_mode()?;
        while !self.should_quit(){
            self.update()?;
            self.handle_events()?;
        }
        // println!("{:?}", self.lines);
        Ok(())
    }
}

