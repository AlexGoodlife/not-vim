use crate::editor::buffer::Viewport;
use crate::editor::is_seperator;
use crate::editor::Editor;
use crate::editor::Mode;
use crate::editor::MoveInfo;
use crate::editor::TABSTOP;
use crate::ui::Component;
use crate::ui::EditorBuffer;
use crate::ui::Gutter;
use crate::ui::MessagesComponent;
use std::io::Stdout;
use std::io::Write;
use std::mem;
use std::time::Duration;

use crate::editor::buffer::RenderBuffer;
use crate::styles::*;
use crossterm::cursor;
use crossterm::event;
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
const INSERT_TABS: bool = true;

#[derive(PartialEq, Clone, Debug)]
enum Action {
    None,
    DeleteUnresolved,
    Delete(Box<Action>, MoveInfo), // we need to know what actions originated what movements so we
    // can be sure what to do with the info
    ChangeUnresolved,
    Change(Box<Action>, MoveInfo),

    MoveForward,
    MoveBackwards,
    MoveDown,
    MoveUp,
    MoveWord,
    MoveEndWord,
    MoveBackWord,
    PopChar,
    PopBackspace,
    PutNewlineInsert,
    WriteCurrentBuffer,

    MoveToUnresolved,
    MoveTo(char),

    MoveUntilUnresolved,
    MoveUntil(char),

    InsertChar(char),
    SwitchMode(Mode),
    ActOnSelf, // Auxiliary action for commands
}

impl Action {
    pub fn expects_input(&self) -> bool {
        matches!(self, Self::MoveToUnresolved | Self::MoveUntilUnresolved)
    }

    pub fn resolve_char(a: &Self, c: char) -> Self {
        match a {
            Self::MoveToUnresolved => Self::MoveTo(c),
            Self::MoveUntilUnresolved => Self::MoveUntil(c),
            _ => a.clone(),
        }
    }
    pub fn resolve_movement(a: &Self, action: Action, movement: MoveInfo) -> Self {
        match a {
            Self::DeleteUnresolved => Self::Delete(Box::new(action), movement),
            Self::ChangeUnresolved => Self::Change(Box::new(action), movement),
            _ => a.clone(),
        }
    }
}
#[derive(PartialEq, Clone, Debug)]
enum Motion {
    Single(Action),
    Command(Box<(Action, Motion)>),
    Repeating(Box<(usize, Motion)>),
}

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
    waiting_input: Option<Action>,
    waiting_action: Option<(Option<usize>, Action)>, // we need to store the repeater state when action as input
    repeater: Option<usize>,
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
            waiting_input: None,
            repeater: None,
            waiting_action: None,
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

    fn handle_insert_keys(&mut self, ev: event::KeyEvent) -> anyhow::Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char(character),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::InsertChar(character)));
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::SwitchMode(Mode::Normal)));
                queue!(self.stdout, crossterm::cursor::SetCursorStyle::SteadyBlock)?
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::PutNewlineInsert));
            }
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::PopBackspace));
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                if INSERT_TABS {
                    self.handle_motions(Motion::Single(Action::InsertChar('\t')));
                } else {
                    for _ in 0..TABSTOP {
                        self.handle_motions(Motion::Single(Action::InsertChar(' ')));
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveBackwards));
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveForward));
            }
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveUp));
            }
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveDown));
            }
            _ => (),
        }
        Ok(())
    }

    // match action{
    fn match_action(&mut self, action: Action, amount: usize) -> (Action, Option<MoveInfo>) {
        match action {
            // We initialize with pos but could be anything
            Action::ActOnSelf => (
                action,
                Some(MoveInfo {
                    start_pos: self.editor.cursor_pos,
                    end_pos: self.editor.cursor_pos,
                }),
            ),
            Action::MoveForward => (action, Some(self.editor.move_cursor_right(amount))),
            Action::MoveBackwards => (action, Some(self.editor.move_cursor_left(amount))),
            Action::MoveUp => (action, Some(self.editor.move_cursor_up(amount))),
            Action::MoveDown => (action, Some(self.editor.move_cursor_down(amount))),
            Action::MoveTo(c) => (action, Some(self.editor.move_to(c, amount, 1))),
            Action::MoveUntil(c) => (action, Some(self.editor.move_to(c, amount, 0))),
            Action::MoveWord => {
                //This hack is necessary
                let r = Some(self.editor.move_word(amount));
                if self.editor.cursor_pos.0 != 0 || is_seperator(self.editor.character_at_cursor())
                {
                    self.editor.move_cursor_right(1);
                }
                return (action, r);
            }
            Action::MoveEndWord => (action, Some(self.editor.move_end_word(amount))),
            Action::MoveBackWord => (action, Some(self.editor.move_end_word_backwards(amount))),
            Action::PopChar => {
                self.editor.pop_char();
                (action, None)
            }
            Action::PopBackspace => {
                self.editor.pop_backspace();
                (action, None)
            }
            Action::InsertChar(c) => {
                self.editor.put_char(c);
                (action, None)
            }
            Action::Delete(ref a, ref movement) => {
                let m = movement.get_ordered();
                if matches!(**a, Action::MoveUp | Action::MoveDown | Action::ActOnSelf) {
                    self.editor.delete_lines(m.clone());
                } else {
                    self.editor.delete_selection(movement.clone());
                }
                self.editor.move_cursor_to(m.start_pos.0, m.start_pos.1);
                (action, None)
            }
            Action::Change(ref a, ref movement) => {
                let m = movement.get_ordered();
                if matches!(**a, Action::MoveUp | Action::MoveDown | Action::ActOnSelf) {
                    self.editor.delete_lines(m.clone());
                } else {
                    self.editor.delete_selection(movement.clone());
                }
                self.editor.move_cursor_to(m.start_pos.0, m.start_pos.1);
                self.match_action(Action::SwitchMode(Mode::Insert), 1);
                (action, None)
            },
            Action::PutNewlineInsert => {
                self.editor.put_newline();
                (action, None)
            }
            Action::SwitchMode(ref mode) => {
                match mode {
                    Mode::Normal => {
                        self.editor.move_cursor_left(1);
                        self.editor.move_cursor_left(1);
                        self.editor.mode = Mode::Normal;
                        self.editor.move_cursor_right(1);
                    }
                    Mode::Insert => {
                        self.editor.mode = Mode::Insert;
                    }
                }
                (action, None)
            }
            Action::WriteCurrentBuffer => {
                self.editor
                    .write_current_buffer()
                    .expect("Writing to buffer failed");
                (action, None)
            }
            Action::MoveToUnresolved
            | Action::MoveUntilUnresolved
            | Action::DeleteUnresolved
            | Action::ChangeUnresolved
            | Action::None => (action, None),
        }
    }

    // Possibly the worst implementation of motions that could exist
    fn flush_motions(&mut self, motion: Motion, quantifier: usize) -> (Action, Option<MoveInfo>) {
        match motion {
            Motion::Command(c) => {
                for _ in 0..quantifier {
                    let movement = self.flush_motions(c.1.clone(), 1);
                    if let Some(mov) = movement.1 {
                        let a = Action::resolve_movement(&c.0, movement.0, mov);
                        self.match_action(a, quantifier);
                    }
                }
            }
            Motion::Single(ref a) => {
                if a.expects_input() {
                    self.waiting_input = Some(a.clone());
                    return (a.clone(), None);
                }
                return self.match_action(a.clone(), quantifier);
            }
            Motion::Repeating(m) => return self.flush_motions(m.1, m.0),
        }
        (Action::None, None)
    }

    fn handle_motions(&mut self, mut motion: Motion) {
        if self.repeater != None {
            motion = Motion::Repeating(Box::new((self.repeater.unwrap(), motion.clone())));
            self.repeater = None;
        }

        if let Some(ref a) = self.waiting_action {
            let binding = a.clone();
            self.waiting_action = None;
            if let Some(repeater) = binding.0 {
                self.handle_motions(Motion::Repeating(Box::new((
                    repeater,
                    Motion::Command(Box::new((binding.1, motion.clone()))),
                ))));
            } else {
                self.handle_motions(Motion::Command(Box::new((binding.1, motion.clone()))));
            }
            return;
        }

        self.flush_motions(motion, 1); // 1 by default
    }

    fn handle_waiting_inputs(&mut self, ev: event::KeyEvent) -> anyhow::Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                // Reset state on Esc
                self.waiting_input = None;
                self.waiting_action = None;
                self.repeater = None;
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                if let Some(a) = &self.waiting_input {
                    let action = a.clone();
                    self.waiting_input = None;
                    self.handle_motions(Motion::Single(Action::resolve_char(&action, c)));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_normal_keys(&mut self, ev: event::KeyEvent) -> anyhow::Result<()> {
        if self.waiting_input.is_some() {
            return self.handle_waiting_inputs(ev);
        }
        match ev {
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                // Reset state on Esc
                self.waiting_input = None;
                self.waiting_action = None;
                self.repeater = None;
            }
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.quit = true;
            }
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.waiting_input = Some(Action::MoveToUnresolved);
            }
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.waiting_input = Some(Action::MoveUntilUnresolved);
            }
            KeyEvent {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveDown));
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveUp));
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveBackwards));
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveForward));
            }
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::PopChar));
            }
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::SwitchMode(Mode::Insert)));
                queue!(self.stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)?
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::SwitchMode(Mode::Insert)));
                self.handle_motions(Motion::Single(Action::MoveForward));
                queue!(self.stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)?
            }
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveWord));
            }
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveBackWord));
            }
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::MoveEndWord));
            }
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(Motion::Single(Action::WriteCurrentBuffer));
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_waiting_command(Action::DeleteUnresolved);
            }
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_waiting_command(Action::ChangeUnresolved);
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                //We only modify a single quantifier
                if !matches!(c, '0'..='9') {
                    return Ok(());
                }

                if let Some(ref mut repeater) = self.repeater {
                    let mut n_str = repeater.to_string();
                    let c_str = c.to_string();
                    n_str.push_str(&c_str);
                    *repeater = n_str.parse::<usize>().unwrap_or(0);
                } else {
                    self.repeater = Some(c.to_digit(10).unwrap_or(1) as usize);
                }
            }
            _ => {
                // We input something wrong, we should clear the repeater
                self.repeater = None;
                ()
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> anyhow::Result<()> {
        if poll(Duration::from_millis(16))? {
            match read()? {
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
                Event::Key(ev) => match self.editor.mode {
                    Mode::Normal => self.handle_normal_keys(ev)?,
                    Mode::Insert => self.handle_insert_keys(ev)?,
                },
                _ => println!("Some other event"),
            }
        }
        Ok(())
    }

    fn handle_waiting_command(&mut self, a: Action) {
        if let Some(action) = &self.waiting_action {
            if a == action.1 {
                return self.handle_motions(Motion::Single(Action::ActOnSelf));
            }
        }
        if let Some(repeater) = self.repeater {
            self.waiting_action = Some((Some(repeater), a));
        } else {
            self.waiting_action = Some((None, a));
        }
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
