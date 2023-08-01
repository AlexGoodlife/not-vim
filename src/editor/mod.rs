use std::io::{stdout, Write};
use std::time::Duration;
use crossterm::{ execute, terminal, terminal::enable_raw_mode, terminal::disable_raw_mode,Result, style,event, queue
};

mod line;

use event::{KeyCode, KeyEvent, KeyModifiers};

use event::{read, poll,Event};
use crate::editor::line::Line;

pub struct EditorFlags{
    quit : bool,
    recompute : bool,
}

pub struct Editor{
    output : std::io::Stdout,
    buffer : String,
    lines : Vec<Line>,
    pub flags : EditorFlags,
}

impl EditorFlags{
    pub fn new() -> EditorFlags{
        EditorFlags{
            quit : false,
            recompute : true,
        }
    }
}

impl Editor{
    pub fn new() -> Editor{
        let initial_text = "Hello my name is\r\nSomething\r\n";
        Editor{
            flags : EditorFlags::new(),
            buffer : String::from(initial_text),
            output : stdout(),
            lines : Line::compute_lines(initial_text),
        }
    }

    fn recompute_lines(&mut self){
        self.lines = Line::compute_lines(&self.buffer);
    }

    fn get_content(&mut self) -> String{
        let mut result = String::new();
        let mut i = 1;
        for line in &self.lines{
            result.push_str(&i.to_string());
            result.push(' ');
            let to_append = self.buffer.chars().skip(line.start).take(line.size).collect::<String>();
            // println!("{:?}", to_append);
            result.push_str(&to_append);
            i += 1;
        }
        result
    }

    fn update_editor(&mut self) -> Result<()>{
        if self.flags.recompute {
            queue!(self.output, terminal::Clear(terminal::ClearType::All))?;
            queue!(self.output, crossterm::cursor::MoveTo(0,0))?;
            queue!(self.output, crossterm::cursor::Hide)?;
            self.recompute_lines();
            let content = self.get_content();
            queue!(self.output, style::Print(content))?;
            // queue!(self.output, style::Print(self.buffer.as_str()))?;
            queue!(self.output, crossterm::cursor::Show)?;
            self.output.flush()?;
            self.flags.recompute = false;
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
                modifiers: KeyModifiers::NONE,
            } => {
                    self.buffer.push(character);
                    self.flags.recompute = true;
                }
            _ => println!("{:?}", ev.code)
        }
        Ok(())
    }

    fn should_quit(&self) -> bool{
        self.flags.quit
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

    fn close(&mut self) ->Result<()>{
        disable_raw_mode()?;
        execute!(self.output, terminal::Clear(terminal::ClearType::All))?;
        execute!(self.output, terminal::LeaveAlternateScreen)?;
        Ok(())
    }

    pub fn run(&mut self) -> Result<()>{
        execute!(self.output, terminal::EnterAlternateScreen)?;
        enable_raw_mode()?;
        while !self.should_quit() {
            self.update_editor()?;
            self.handle_events()?;
        }
        self.close()?;
        println!("{:?}", self.lines);
        Ok(())
    }
}
