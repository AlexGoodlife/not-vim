use event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use std::io::{Stdout, stdout, Write};
use crossterm::style::Stylize;
use event::{read, poll,Event};
use crossterm::{ execute, terminal, terminal::enable_raw_mode, terminal::disable_raw_mode,Result, style,event, queue, cursor,terminal::size
};
use crate::editor::Editor;
use crate::util;

const DEFAULT_PADDING:usize = 3;

const DEBUG:bool = true;

pub struct EditorFlags{
    quit : bool,
    recompute : bool,
    refresh : bool,
}

impl EditorFlags{
    pub fn new() -> EditorFlags{
        EditorFlags{
            quit : false,
            recompute : true,
            refresh : true,
        }
    }
}

struct Cursor{
    pub x : u16,
    pub y : u16,
}

pub struct Client{
    content : String,
    cursor : Cursor,
    editor : Editor,
    flags : EditorFlags,
    output : Stdout,
    window_dimensions : (u16,u16),
    window_offset : usize,
    text_dimensions : (u16,u16),
}

impl Drop for Client{
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        if DEBUG{

            self.editor.lines.iter().for_each(|line|println!("{:?}",line));
            println!("index {:?}", self.editor.cursor_index);
            // println!("buffer len {:?}", self.editor.get_buffer().len());
            // println!("{:?}", self.editor.lines);
            // println!("{:?}", self.editor.cursor_index);
            // println!("{:?}", self.window_offset);
            // println!("{:?}", self.editor.lines.len());
            // self.editor.lines.iter().enumerate().for_each(|(i,line)| {
            //     print!("{} ", i);
            //     println!("{:?}", line);
            // }
            // );
        }
        else{
            execute!(self.output, terminal::Clear(terminal::ClearType::All)).unwrap();
            execute!(self.output, terminal::LeaveAlternateScreen).unwrap();
        }
    }

}

impl Client{

    pub fn new() -> Self{
        let status_bar_size = 1;
        let size = size().unwrap_or((0,0));
        Self{
            content : String::new(),
            cursor : Cursor{x : 0, y : 0},
            editor : Editor::new(),
            flags : EditorFlags::new(),
            output : stdout(),
            window_dimensions : size,
            window_offset : 0,
            text_dimensions: (size.0, size.1 - status_bar_size),
        }
    }

    pub fn open_file(&mut self, path: &str){
        self.editor.open_file(path);
    }

    fn should_quit(&self) -> bool{
        self.flags.quit
    }

    fn should_recompute(&self) -> bool{
        self.flags.recompute
    }

    fn move_cursor_to_index(&mut self){
        let index = self.editor.cursor_index;
        // let current_line = self.editor.lines.iter().enumerate().find(|line| index >=line.1.start && index <= line.1.end).unwrap();
        let current_line = self.editor.get_cursor_line().unwrap();
        let row = current_line.0;
        let mut collumn = std::cmp::max(0,index - current_line.1.start);

        let left_padding = std::cmp::max(DEFAULT_PADDING,util::digits(self.editor.lines.len()));

        self.window_offset = std::cmp::min(self.window_offset,row);
        if row >= self.window_offset + self.text_dimensions.1 as usize{
            self.window_offset = row as usize - self.text_dimensions.1 as usize + 1;
        }
        //For now we offset collumns by 2
        // let right_padding = util::digits(row)+1;
        let right_padding = 1;
        collumn += left_padding + right_padding;
        self.cursor.x = collumn as u16;
        // self.y = 0 as u16;
        self.cursor.y = (row as usize - self.window_offset as usize) as u16;
    }

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
        
        let left_padding = std::cmp::max(DEFAULT_PADDING,util::digits(self.editor.lines.len()));
        
        let rows = self.text_dimensions.1 as usize;
        for i in 0..rows{
            let current_size = util::digits(i + self.window_offset+1);
            let idx = i + self.window_offset;
            if idx >= self.editor.lines.len(){
                result.push_str(&'~'.dark_grey().to_string());
                // if i < rows-1 {
                    result.push_str("\r\n");
                // }
                continue;
            }

            for _pad in 0..(left_padding - current_size){
                result.push(' ');
            }
            result.push_str(&(idx+1).to_string().dark_grey().to_string());
            result.push(' ');
            let mut to_append = buffer.chars().skip(self.editor.lines[idx].start).take(self.editor.lines[idx].size).collect::<String>();
            // weird stuff
            if let Some(character) = to_append.pop(){
                if character != '\n'{
                    to_append.push(character);
                }
                // if i < rows-1 {
                    to_append.push('\r');
                    to_append.push('\n');
                // }
            }
            else{
                // if i < rows-1{
                    to_append.push_str("\r\n");
                // }
            }
            result.push_str(&to_append);
        }
    }

    fn update(&mut self) -> Result<()>{
        // self.cursor.move_to_index(&self.editor.lines, self.editor.cursor_index,window_x.into(), window_y.into(), &mut self.window_offset);
        if self.should_recompute() {
            self.editor.compute_lines(self.window_offset, self.text_dimensions.1.into());
            //this needs to be better implemented later
            self.move_cursor_to_index();
            self.get_content();
            queue!(self.output, terminal::Clear(terminal::ClearType::All))?;
            // queue!(self.output, cursor::SavePosition)?;
            queue!(self.output, cursor::Hide)?;
            queue!(self.output, cursor::MoveTo(0,0))?;

            queue!(self.output, style::Print(&self.content))?;
            queue!(self.output, style::Print(&self.editor.get_status()))?;

            // queue!(self.output, cursor::RestorePosition)?;
            queue!(self.output, crossterm::cursor::Show)?;
            self.output.flush()?;
            // self.cursor.move_to_index(&self.editor.lines, self.editor.cursor_index,window_x.into(), window_y.into(), &mut self.window_offset);
            self.flags.recompute = false;
        }
        let old_offset = self.window_offset.clone();
        self.move_cursor_to_index();
        if old_offset != self.window_offset{
            self.flags.recompute = true;
        }
        self.move_cursor()?;
        self.output.flush()?;
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
            KeyEvent {
                code: KeyCode::Right,
                modifiers : KeyModifiers::NONE
            } =>{
                    self.editor.move_cursor_right();
                }
            KeyEvent {
                code: KeyCode::Left,
                modifiers : KeyModifiers::NONE
            } =>{
                    self.editor.move_cursor_left();
                }
            KeyEvent {
                code: KeyCode::Up,
                modifiers : KeyModifiers::NONE
            } =>{
                    self.editor.move_cursor_up();
                }
            KeyEvent {
                code: KeyCode::Down,
                modifiers : KeyModifiers::NONE
            } =>{
                    self.editor.move_cursor_down();
                }
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers : KeyModifiers::CONTROL
            } =>{
                    self.editor.save_file();
                    //temporary
                    self.flags.recompute = true;
                }
            _ => ()
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()>{
        if poll(Duration::from_millis(16))? {
            match read()?{
                Event::Key(ev) => self.handle_keys(ev)?,
                _ => println!("Some other event"),
            }

        }
        Ok(())
    }
    pub fn run(&mut self) -> Result<()>{
        if !DEBUG {
            execute!(self.output, terminal::EnterAlternateScreen)?;
        }
        enable_raw_mode()?;
        while !self.should_quit(){
            self.update()?;
            self.handle_events()?;
        }
        Ok(())
    }
}

