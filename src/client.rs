use crate::editor::buffer::Viewport;
use crate::editor::Editor;
use crate::ui::edit_buffer::EditorBuffer;
use crate::ui::Component;
use crate::ui::Gutter;
use crate::ui::MessagesComponent;
use std::io::Stdout;
use std::io::Write;
use std::mem;
use std::time::Duration;

use crate::editor::buffer::RenderBuffer;
use crate::styles::*;
use crossterm::cursor;
use crossterm::event::poll;
use crossterm::event::read;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyEventState;
use crossterm::event::KeyModifiers;
use crossterm::execute;
use crossterm::queue;
use crossterm::style::ContentStyle;
use crossterm::style::PrintStyledContent;
use crossterm::terminal;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;

const DEBUG: bool = false;

pub struct Client {
    stdout: Box<dyn Write>,
    quit: bool,
    // window_dimensions: (u16, u16),
    curr_buffer: RenderBuffer,
    next_buffer: RenderBuffer,
    cursor_pos: (u16, u16),
    pub editor: Editor,
    ui_components: Vec<Box<dyn Component>>,
    active_compontent_index: usize,
}

impl Client {
    pub fn new(stdout: Stdout, dimensions: (u16, u16)) -> Client {
        let w = dimensions.0 as usize;
        let h = dimensions.1 as usize;
        let mut result = Client {
            stdout: Box::new(stdout),
            quit: false,
            // window_dimensions: (w as u16, (h.saturating_sub(1)) as u16), // for gutter
            curr_buffer: RenderBuffer::new(w, h),
            next_buffer: RenderBuffer::new(w, h),
            cursor_pos: (0, 0),
            editor: Editor::new(),
            ui_components: Vec::new(),
            active_compontent_index: 0,
        };
        let messages_viewport = Viewport {
            pos: (0, h.saturating_sub(1)),
            width: w,
            height: 1,
        };
        let buffer_viewport = Viewport {
            pos: (0, 0),
            width: w,
            height: h.saturating_sub(2),
        };
        let gutter_viewport = Viewport {
            pos: (0, h.saturating_sub(2)),
            width: w,
            height: 1,
        };
        result.ui_components.push(Box::new(EditorBuffer::new(
            buffer_viewport,
            Box::new(|w, h| {
                let x = 0;
                let y = 0;
                let width = w;
                let height = h.saturating_sub(2);
                Viewport {
                    pos: (x, y),
                    width,
                    height,
                }
            }),
        )));

        result.ui_components.push(Box::new(Gutter::new(
            gutter_viewport,
            Box::new(|w, h| Viewport {
                pos: (0, h.saturating_sub(2)),
                width: w,
                height: 1,
            }),
        )));

        result.ui_components.push(Box::new(MessagesComponent::new(
            messages_viewport,
            Box::new(|w, h| Viewport {
                pos: (0, h.saturating_sub(1)),
                width: w,
                height: 1,
            }),
        )));
        result
    }

    fn resize_components(&mut self, window_w: usize, window_h: usize) {
        for c in self.ui_components.iter_mut() {
            c.resize(window_w, window_h);
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        execute!(self.stdout, terminal::EnterAlternateScreen)?;
        execute!(self.stdout, crossterm::cursor::SetCursorStyle::SteadyBlock)?;
        enable_raw_mode()?;
        while !self.quit {
            self.handle_events()?;
            self.update()?;
        }
        Ok(())
    }

    fn render_to_screen(&mut self) -> anyhow::Result<()> {
        let diff = self.curr_buffer.diff(&self.next_buffer);

        queue!(self.stdout, cursor::Hide)?;
        for patch in diff {
            let (x, y) = patch.pos;
            queue!(self.stdout, cursor::MoveTo(x as u16, y as u16))?;

            let styled_content = ContentStyle::apply(patch.style, &patch.content);
            queue!(self.stdout, PrintStyledContent(styled_content))?;
        }
        mem::swap(&mut self.next_buffer, &mut self.curr_buffer);
        self.next_buffer.clear_buffer(BLACK);
        Ok(())
    }

    fn update_components(&mut self) {
        for c in self.ui_components.iter_mut() {
            c.draw(&mut self.next_buffer, &mut self.editor)
        }
        let current_component = &self.ui_components[self.active_compontent_index];
        let (viewport_x, viewport_y) = current_component.get_viewport().pos;

        let (new_x, new_y) =
            self.ui_components[self.active_compontent_index].update_cursor(&mut self.editor);
        self.cursor_pos.0 = viewport_x as u16 + new_x;
        self.cursor_pos.1 = viewport_y as u16 + new_y;
    }

    fn update(&mut self) -> anyhow::Result<()> {
        self.update_components();
        self.render_to_screen()?;

        queue!(
            self.stdout,
            cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
        )?;
        queue!(self.stdout, crossterm::cursor::Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    fn handle_keys(&mut self, ev: KeyEvent) -> anyhow::Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_events(&mut self) -> anyhow::Result<()> {
        if poll(Duration::from_millis(16))? {
            let event = read()?;
            match event {
                Event::Resize(w, h) => {
                    self.next_buffer = RenderBuffer::new(w.into(), h.into());
                    self.curr_buffer = RenderBuffer::new(w.into(), h.into());
                    self.cursor_pos = (0, 0);
                    // self.window_dimensions = (w, h.saturating_sub(1)); // -1 for gutter
                    self.resize_components(w.into(), h.into());
                    self.stdout.flush()?;
                    execute!(
                        self.stdout,
                        cursor::MoveTo(self.cursor_pos.0, self.cursor_pos.1)
                    )?;
                    execute!(self.stdout, terminal::Clear(terminal::ClearType::All))?;
                }
                Event::Key(ev) => self.handle_keys(ev)?,
                _ => println!("Some other event"),
            }
            self.ui_components[self.active_compontent_index].handle_events(
                &mut self.stdout,
                &mut self.editor,
                event,
            )?;
        }
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(self.stdout, terminal::Clear(terminal::ClearType::All)).unwrap();
        execute!(self.stdout, terminal::LeaveAlternateScreen).unwrap();
        if DEBUG {
            let mut i = 0;
            for cell in self.curr_buffer.data.clone() {
                if cell.character == ' ' {
                    continue;
                }
                if cell.character == '\n' {
                    log::info!("NEWLINE");
                    continue;
                }
                if cell.character == '\r' {
                    log::info!("CARRIAGE RETURN");
                    continue;
                }
                log::info!("{}", cell.character);
                i = i + 1;
                if i == 100 {
                    break;
                }
            }
        }
    }
}
