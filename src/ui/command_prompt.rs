use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use crate::{
    editor::buffer::{Cell, Viewport},
    styles::{command_style, default_text_style},
};

use super::{text_prompt::TextPrompt, ClientAction, Component};

pub struct CommandPrompt {
    prompt: TextPrompt,
    viewport: Viewport,
    resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
    side_scroll: usize,
    left_offset: usize,
    right_offset: usize,
}

impl Component for CommandPrompt {
    fn update_cursor(&mut self, _editor: &mut crate::editor::Editor) -> (i64, i64) {
        let prompt_x = self.prompt.get_cursor();

        if prompt_x >= self.viewport.width.saturating_sub(self.right_offset) + self.side_scroll - self.left_offset {
            self.side_scroll +=
                prompt_x - (self.viewport.width.saturating_sub(self.right_offset) + self.side_scroll - self.left_offset)
        }

        if prompt_x < self.side_scroll + self.left_offset {
            self.side_scroll = self
                .side_scroll
                .saturating_sub((self.side_scroll).saturating_sub(prompt_x));
        }
        (
            (self.left_offset + prompt_x).saturating_sub(self.side_scroll) as i64,
            (self.viewport.height/2) as i64,
        )
    }

    fn draw(
        &mut self,
        buffer: &mut crate::editor::buffer::RenderBuffer,
        _editor: &mut crate::editor::Editor,
    ) {
        //Draw a box
        let w = self.viewport.width;
        let h = self.viewport.height;
        let mut vec = Vec::new();
        for _ in 0..w {
            vec.push(Cell::with_style(' ', command_style()));
        }
        let vec_len = vec.len();

        for i in 0..h {
            buffer.put_cells(&vec, (0,i as i64), &self.viewport);
        }

        for i in 0..w{
            vec[i] = Cell::with_style('─', command_style())
        }
        vec[0] = Cell::with_style('┌', command_style());
        vec[vec_len-1] = Cell::with_style('┐', command_style());

        buffer.put_cells(&vec, (0,0 ), &self.viewport);

        for i in 0..w {
            vec[i]= Cell::with_style(' ', command_style());
        }

        let content = self.prompt.get_content();
        //Draw the text in the box
        for (i, c) in content.chars().skip(self.side_scroll).take(vec.len() - (self.right_offset + self.left_offset)).enumerate() {
            vec[i + self.left_offset] = Cell::with_style(c, default_text_style(false));
        }
        vec[0] = Cell::with_style('│', command_style());
        vec[std::cmp::min(vec_len-1, 1)] = Cell::with_style('>', command_style());
        vec[vec_len-1] = Cell::with_style('│', command_style());
        buffer.put_cells(&vec, (0,(h/2) as i64), &self.viewport);

        for i in 0..w {
            vec[i] = Cell::with_style('─', command_style());
        }
        vec[0] = Cell::with_style('└', command_style());
        vec[vec_len-1] = Cell::with_style('┘', command_style());
        buffer.put_cells(&vec, (0,(h-1) as i64), &self.viewport);
    }

    fn get_viewport(&self) -> &Viewport {
        &self.viewport
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.viewport = (self.resize_callback)(w, h);
    }

    fn set_resize_callback(&mut self, c: Box<dyn Fn(usize, usize) -> Viewport>) {
        self.resize_callback = c;
    }

    fn handle_events(
        &mut self,
        stdout: &mut Box<dyn std::io::Write>,
        editor: &mut crate::editor::Editor,
        event: crossterm::event::Event,
    ) -> anyhow::Result<(bool, super::ClientAction)> {
        match event {
            crossterm::event::Event::Key(k) => match k {
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                } => {
                    self.prompt.push_char(c);
                }
                KeyEvent {
                    code: KeyCode::Backspace,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                } => {
                    self.prompt.pop_char();
                }
                KeyEvent {
                    code: KeyCode::Left,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                } => {
                    self.prompt.move_cursor_left(1);
                }
                KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                } => {
                    self.prompt.move_cursor_right(1);
                }
                KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                } => return Ok((true, CommandPrompt::process_command(self.prompt.get_content()))),
                KeyEvent {
                    code: KeyCode::Esc,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                } => return Ok((true, ClientAction::None)),
                _ => {}
            },
            _ => {}
        }
        Ok((false, ClientAction::None))
    }

    fn is_interactive(&self) -> bool {
        true
    }
}

impl CommandPrompt {
    pub fn new(viewport: Viewport, resize_callback: Box<dyn Fn(usize, usize) -> Viewport>) -> Self {
        Self {
            viewport,
            resize_callback,
            prompt: TextPrompt::new(),
            side_scroll: 0,
            left_offset: 3,
            right_offset: 2,
        }
    }

    pub fn process_command(raw_string: &str) -> ClientAction {
        let argv: Vec<&str> = raw_string.split_whitespace().collect();
        match argv[0] {
           "q" => ClientAction::Quit,
            _ => ClientAction::None
        }
    }

}
