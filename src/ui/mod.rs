use std::io::Write;

pub mod edit_buffer;
pub mod command_prompt;
pub mod text_prompt;
use crossterm::event::Event;

use crate::{
    editor::{
        buffer::{RenderBuffer, Viewport},
        Editor, EditorStatus,
    },
    styles::{default_text_style, mode_style},
};

pub fn resize_viewport(viewport: &Viewport, w: usize, h: usize) -> Viewport {
    Viewport {
        pos: viewport.pos,
        width: w,
        height: h,
    }
}

// Actions to be bubbled up and performed on the client
#[derive(PartialEq, Clone, Debug)]
pub enum ClientAction {
    None,
    Quit,
    OpenBuffer(String),
    SaveCurrentBuffer,
    CloseBuffer,
    NextBuffer,
    PreviousBuffer,
    OpenCommandPrompt,
    Multiple(Vec<ClientAction>),
}

//Perfect place for a Macro to generate new methods for me, split the resize stuff into a seperate
//trait and have all my components derive it, boom done, for now we do it by hand
pub trait Component {
    fn update_cursor(&mut self, editor: &mut Editor) -> (i64, i64);
    fn draw(&mut self, buffer: &mut RenderBuffer, editor: &mut Editor);
    fn get_viewport(&self) -> &Viewport;
    fn resize(&mut self, w: usize, h: usize);
    fn set_resize_callback(&mut self, c: Box<dyn Fn(usize, usize) -> Viewport>);
    fn handle_events(
        &mut self,
        stdout: &mut Box<dyn Write>,
        editor: &mut Editor,
        event: Event,
    ) -> anyhow::Result<(bool, ClientAction)>; // return if component no longer needs to exist
    fn is_interactive(&self) -> bool;
}


pub struct Gutter {
    gutter_viewport: Viewport,
    resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
}

impl Gutter {
    pub fn new(
        gutter_viewport: Viewport,
        resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
    ) -> Gutter {
        Gutter {
            gutter_viewport,
            resize_callback,
        }
    }
}

impl Component for Gutter {
    fn update_cursor(&mut self, _editor: &mut Editor) -> (i64, i64) {
        (0, 0) // Gutter doesn't get any cursors on it
    }

    fn draw(&mut self, buffer: &mut RenderBuffer, editor: &mut Editor) {
        // Todo refactor all of this bs
        let status = EditorStatus::from_editor(&editor);
        // We draw the gutter across the entire buffer
        let mode = format!(" {} ", status.mode.to_string());
        let mode_len = mode.chars().count();

        let changes = if status.has_changes { " [+]" } else { "" };
        let name = format!("{}{}", status.curr_buffer, changes);
        let name_len = name.chars().count();

        let spacing_size = 3; // random spaces between things
        let positions = format!(
            "{} B | {}:{} ",
            status.bytes.to_string(),
            status.cursor_pos.1,
            status.cursor_pos.0
        );
        let position_pad =
            std::cmp::max(positions.chars().count() + spacing_size, buffer.width / 20);
        let position = format!("{:>position_pad$}", positions);
        let position_len = position.chars().count();

        //unused anymore but I'm keeping it
        let _padding_len = buffer
            .width
            .saturating_sub(mode_len + position_len + name_len + spacing_size);

        // let y = buffer.height.saturating_sub(1);
        buffer.put_str(
            &mode,
            (0, 0),
            mode_style(&editor.mode),
            &self.gutter_viewport,
        );
        buffer.put_str(
            &name,
            (mode_len + 1, 0),
            default_text_style(false),
            &self.gutter_viewport,
        );
        buffer.put_str(
            &position,
            (buffer.width.saturating_sub(position_len), 0),
            mode_style(&editor.mode),
            &self.gutter_viewport,
        );
    }

    fn get_viewport(&self) -> &Viewport {
        &self.gutter_viewport
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.gutter_viewport = (self.resize_callback)(w, h);
    }

    fn set_resize_callback(&mut self, c: Box<dyn Fn(usize, usize) -> Viewport>) {
        self.resize_callback = c;
    }

    fn handle_events(
        &mut self,
        _stdout: &mut Box<(dyn Write)>,
        _editor: &mut Editor,
        _event: Event,
    ) -> std::result::Result<(bool, ClientAction), anyhow::Error> {
        Ok((false,ClientAction::None))
    }

    fn is_interactive(&self) -> bool {
        false
    }
}

pub struct MessagesComponent {
    viewport: Viewport,
    resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
}

impl MessagesComponent {
    pub fn new(
        viewport: Viewport,
        resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
    ) -> MessagesComponent {
        MessagesComponent {
            viewport,
            resize_callback,
        }
    }
}

impl Component for MessagesComponent {
    fn get_viewport(&self) -> &Viewport {
        &self.viewport
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.viewport = (self.resize_callback)(w, h);
    }

    fn set_resize_callback(&mut self, c: Box<dyn Fn(usize, usize) -> Viewport>) {
        self.resize_callback = c;
    }

    fn update_cursor(&mut self, _editor: &mut Editor) -> (i64, i64) {
        (0, 0)
    }

    fn draw(&mut self, buffer: &mut RenderBuffer, editor: &mut Editor) {
        buffer.put_str(
            &editor.message,
            (0, 0),
            default_text_style(false),
            &self.viewport,
        );
    }

    fn handle_events(
        &mut self,
        _stdout: &mut Box<(dyn Write)>,
        _editor: &mut Editor,
        _event: Event,
    ) -> std::result::Result<(bool, ClientAction), anyhow::Error> {
        Ok((false, ClientAction::None))
    }
    fn is_interactive(&self) -> bool {
        false
    }
}
