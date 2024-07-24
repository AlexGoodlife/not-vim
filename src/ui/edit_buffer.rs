use std::io::Write;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    queue,
};

const YANK_HIGHLIGHT_FAMES: usize = 15;
const INSERT_TABS: bool = true;
use crate::{
    editor::{
        buffer::{Cell, RenderBuffer, Viewport},
        Editor, Mode, MoveInfo, TABSTOP,
    },
    styles::{default_line_number_style, default_text_style, highlighted_text},
};

use super::Component;
#[derive(PartialEq, Clone, Debug)]
enum Action {
    None,
    DeleteUnresolved,
    Delete(Box<Action>, MoveInfo), // we need to know what actions originated what movements so we
    // can be sure what to do with the info
    ChangeUnresolved,
    Change(Box<Action>, MoveInfo),

    CopyUnresolved,
    Copy(Box<Action>, MoveInfo),

    CenterUnresolved,
    Center(Box<Action>, MoveInfo),

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
    DeleteVisualMode,
    ChangeVisualMode,
    MoveEndOfLine,
    AppendEndOfLine,
    Paste,
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
            Self::CenterUnresolved => Self::Center(Box::new(action), movement),
            Self::CopyUnresolved => Self::Copy(Box::new(action), movement),
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

pub struct EditorBuffer {
    top_index: usize,
    left_offset: usize, // For line numbers,
    side_scroll: usize,
    viewport: Viewport,
    resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
    waiting_input: Option<Action>,
    waiting_action: Option<(Option<usize>, Action)>, // we need to store the repeater state when action as input
    repeater: Option<usize>,
    highlighted_selection: Option<MoveInfo>,
    elapsed_frames: usize,
}

impl EditorBuffer {
    pub fn new(
        viewport: Viewport,
        resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
    ) -> EditorBuffer {
        EditorBuffer {
            top_index: 0,
            left_offset: 3, // space number |
            viewport,
            side_scroll: 0,
            resize_callback,
            waiting_input: None,
            waiting_action: None,
            repeater: None,
            highlighted_selection: None,
            elapsed_frames: 0,
        }
    }

    fn is_in_selection(x: usize, y: usize, selection: &MoveInfo) -> bool {
        // if x == 1 && y == 0 {
        //     log::info!("x {} y {} selection {:?}", x, y, selection);
        // }
        let (start_x, start_y) = selection.start_pos;
        let (end_x, end_y) = selection.end_pos;
        if start_y == end_y {
            return y == start_y && (x >= start_x && x <= end_x);
        }
        if y == start_y {
            return x >= start_x;
        }
        if y == end_y {
            return x <= end_x;
        }
        y > start_y && y < end_y
    }
    pub fn draw_lines(&mut self, render_buffer: &mut RenderBuffer, editor: &mut Editor) {
        //Fill current_line with different highlight
        render_buffer.put_str(
            &" ".repeat(self.viewport.width.saturating_sub(self.left_offset)),
            (
                self.left_offset,
                editor.cursor_pos.1.saturating_sub(self.top_index),
            ),
            default_text_style(true),
            &self.viewport,
        );
        for (i, line) in editor.buffer.lines.iter().skip(self.top_index).enumerate() {
            if i >= self.viewport.height as usize {
                break;
            }

            // Transform \t into appropriate amount of spaces, using size instead of len() to avoid
            // counting string length everytime
            let mut s = String::new();
            let mut size = 0;
            let mut cells: Vec<Cell> = Vec::new();
            let l = if line.len() == 0 { " " } else { line }; //  to render empty lines in visual mode

            for (x, c) in l.chars().enumerate() {
                //Draw yanked highlight
                let mut style = match &self.highlighted_selection {
                    Some(selection) => {
                        if Self::is_in_selection(x, i + self.top_index, &selection)
                            && self.elapsed_frames <= YANK_HIGHLIGHT_FAMES
                        {
                            highlighted_text()
                        } else {
                            default_text_style(i + self.top_index == editor.cursor_pos.1)
                        }
                    }
                    None => default_text_style(i + self.top_index == editor.cursor_pos.1),
                };

                style = match &editor.curr_selection {
                    Some(selection) => {
                        if Self::is_in_selection(x, i + self.top_index, &selection.1) {
                            highlighted_text()
                        } else {
                            default_text_style(i + self.top_index == editor.cursor_pos.1)
                        }
                    }
                    None => style
                };

                if c == '\t' {
                    for _ in 0..Editor::get_spaces_till_next_tab(size, TABSTOP) {
                        cells.push(Cell::with_style(' ', style));
                        s.push(' ');
                        size += 1;
                    }
                } else {
                    cells.push(Cell::with_style(c, style));
                    s.push(c);
                    size += 1;
                }
            }
            let skipped = cells
                .into_iter()
                .skip(self.side_scroll)
                .collect::<Vec<Cell>>();
            render_buffer.put_cells(&skipped, (self.left_offset, i), &self.viewport);
        }
    }

    fn draw_line_numbers(&mut self, render_buffer: &mut RenderBuffer, editor: &mut Editor) {
        self.left_offset = editor.buffer.lines.len().to_string().chars().count() + 3; //  3 extra for '|' and a  2 spaces
        for (i, _line) in editor.buffer.lines.iter().skip(self.top_index).enumerate() {
            if i >= self.viewport.height as usize {
                break;
            }

            let num_str = (i + self.top_index + 1).to_string();
            let padding = self.left_offset - 3;
            let padded = format!("{:>padding$} │ ", num_str);

            render_buffer.put_str(
                &padded,
                (0, i),
                default_line_number_style(i + self.top_index == editor.cursor_pos.1),
                &self.viewport,
            );
        }
    }

    // match action{
    fn match_action(
        &mut self,
        stdout: &mut impl Write,
        editor: &mut Editor,
        action: Action,
        amount: usize,
    ) -> Option<MoveInfo> {
        match action {
            // We initialize with pos but could be anything
            Action::ActOnSelf => Some(MoveInfo {
                start_pos: editor.cursor_pos,
                end_pos: editor.cursor_pos,
            }),
            Action::MoveForward => Some(editor.move_cursor_right(amount)),
            Action::MoveBackwards => Some(editor.move_cursor_left(amount)),
            Action::MoveUp => Some(editor.move_cursor_up(amount)),
            Action::MoveDown => Some(editor.move_cursor_down(amount)),
            Action::MoveTo(c) => Some(editor.move_to(c, amount, 1)),
            Action::MoveUntil(c) => Some(editor.move_to(c, amount, 0)),
            Action::MoveWord => {
                //This hack is necessary
                // let r = Some(editor.move_word(amount));
                // if editor.cursor_pos.0 != 0 || is_seperator(editor.character_at_cursor()) {
                // editor.move_cursor_right(1);
                // }
                // return r;
                return Some(editor.move_word(amount));
            }
            Action::MoveEndWord => Some(editor.move_end_word(amount)),
            Action::MoveBackWord => Some(editor.move_end_word_backwards(amount)),
            Action::PopChar => {
                editor.pop_char();
                None
            }
            Action::PopBackspace => {
                editor.pop_backspace();
                None
            }
            Action::InsertChar(c) => {
                editor.put_char(c);
                None
            }
            Action::Paste => {
                editor.paste();
                None
            }
            Action::Copy(ref a, ref movement) => {
                let m = movement.get_ordered();
                if matches!(**a, Action::MoveUp | Action::MoveDown | Action::ActOnSelf) {
                    self.highlighted_selection = Some(editor.copy_lines(m.clone()));
                } else {
                    self.highlighted_selection = Some(editor.copy(movement.clone()));
                }
                editor.move_cursor_to(m.start_pos.0, m.start_pos.1);
                self.elapsed_frames = 0;
                None
            }
            Action::Delete(ref a, ref movement) => {
                let m = movement.get_ordered();
                if matches!(**a, Action::MoveUp | Action::MoveDown | Action::ActOnSelf) {
                    editor.delete_lines(m.clone());
                } else {
                    editor.delete_selection(movement.clone());
                }
                editor.move_cursor_to(m.start_pos.0, m.start_pos.1);
                None
            }
            Action::Change(ref a, ref movement) => {
                let m = movement.get_ordered();
                if matches!(**a, Action::MoveUp | Action::MoveDown | Action::ActOnSelf) {
                    editor.delete_lines(m.clone());
                } else {
                    editor.delete_selection(movement.clone());
                }
                editor.move_cursor_to(m.start_pos.0, m.start_pos.1);
                self.match_action(stdout, editor, Action::SwitchMode(Mode::Insert), 1);
                queue!(stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)
                    .expect("Refactor this out later");
                None
            }
            Action::Center(ref a, ref movement) => {
                if matches!(**a, Action::MoveUp | Action::MoveDown | Action::ActOnSelf) {
                    let to_center = movement.end_pos.1;
                    self.top_index = to_center.saturating_sub(self.viewport.height / 2);
                }
                None
            }
            Action::PutNewlineInsert => {
                editor.put_newline();
                None
            }
            Action::SwitchMode(ref mode) => {
                editor.switch_mode(mode.clone());
                None
            }
            Action::WriteCurrentBuffer => {
                editor
                    .write_current_buffer()
                    .expect("Writing to buffer failed");
                None
            }
            Action::DeleteVisualMode => {
                if let Some(selection) = &editor.curr_selection {
                    let (x, y) = selection.0;
                    editor.delete_selection(selection.1.clone());
                    editor.switch_mode(Mode::Normal);
                    editor.move_cursor_to(x, y);
                }
                None
            }
            Action::ChangeVisualMode => {
                if let Some(selection) = &editor.curr_selection {
                    let (x, y) = selection.0;
                    editor.delete_selection(selection.1.clone());
                    editor.switch_mode(Mode::Insert);
                    editor.move_cursor_to(x, y);
                }
                None
            }
            Action::MoveToUnresolved
            | Action::MoveUntilUnresolved
            | Action::DeleteUnresolved
            | Action::ChangeUnresolved
            | Action::CenterUnresolved
            | Action::CopyUnresolved
            | Action::None => None,
            Action::MoveEndOfLine => Some(editor.move_to_end()),
            Action::AppendEndOfLine => {
                editor.move_to_end();
                editor.switch_mode(Mode::Insert);
                editor.move_cursor_right(1);
                None
            }
        }
    }

    // Possibly the worst implementation of motions that could exist
    fn flush_motions(
        &mut self,
        stdout: &mut impl Write,
        editor: &mut Editor,
        motion: Motion,
        quantifier: usize,
    ) -> (Action, Option<MoveInfo>) {
        match motion {
            Motion::Command(c) => {
                for _ in 0..quantifier {
                    let movement = self.flush_motions(stdout, editor, c.1.clone(), 1);
                    if let Some(mov) = movement.1 {
                        let a = Action::resolve_movement(&c.0, movement.0, mov);
                        self.match_action(stdout, editor, a, quantifier);
                    }
                }
            }
            Motion::Single(ref a) => {
                if a.expects_input() {
                    self.waiting_input = Some(a.clone());
                    return (a.clone(), None);
                }
                return (
                    a.clone(),
                    self.match_action(stdout, editor, a.clone(), quantifier),
                );
            }
            Motion::Repeating(m) => return self.flush_motions(stdout, editor, m.1, m.0),
        }
        (Action::None, None)
    }

    fn handle_motions(&mut self, stdout: &mut impl Write, editor: &mut Editor, mut motion: Motion) {
        if self.repeater != None {
            motion = Motion::Repeating(Box::new((self.repeater.unwrap(), motion.clone())));
            self.repeater = None;
        }

        if let Some(ref a) = self.waiting_action {
            let binding = a.clone();
            self.waiting_action = None;
            if let Some(repeater) = binding.0 {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Repeating(Box::new((
                        repeater,
                        Motion::Command(Box::new((binding.1, motion.clone()))),
                    ))),
                );
            } else {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Command(Box::new((binding.1, motion.clone()))),
                );
            }
            return;
        }

        self.flush_motions(stdout, editor, motion, 1); // 1 by default
    }

    fn handle_waiting_inputs(
        &mut self,
        stdout: &mut impl Write,
        editor: &mut Editor,
        ev: event::KeyEvent,
    ) -> anyhow::Result<()> {
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
                    self.handle_motions(
                        stdout,
                        editor,
                        Motion::Single(Action::resolve_char(&action, c)),
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_keys(
        &mut self,
        stdout: &mut impl Write,
        editor: &mut Editor,
        ev: event::KeyEvent,
    ) -> anyhow::Result<()> {
        match ev {
            KeyEvent {
                code: KeyCode::Char(character),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Single(Action::InsertChar(character)),
                );
            }
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Single(Action::SwitchMode(Mode::Normal)),
                );
                queue!(stdout, crossterm::cursor::SetCursorStyle::SteadyBlock)?
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::PutNewlineInsert));
            }
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::PopBackspace));
            }
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                if INSERT_TABS {
                    self.handle_motions(stdout, editor, Motion::Single(Action::InsertChar('\t')));
                } else {
                    for _ in 0..TABSTOP {
                        self.handle_motions(
                            stdout,
                            editor,
                            Motion::Single(Action::InsertChar(' ')),
                        );
                    }
                }
            }
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveBackwards));
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveForward));
            }
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveUp));
            }
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveDown));
            }
            _ => (),
        }
        Ok(())
    }
    fn handle_normal_keys(
        &mut self,
        stdout: &mut impl Write,
        editor: &mut Editor,
        ev: event::KeyEvent,
    ) -> anyhow::Result<()> {
        if self.waiting_input.is_some() {
            return self.handle_waiting_inputs(stdout, editor, ev);
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
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Single(Action::SwitchMode(Mode::Normal)),
                );
            }
            KeyEvent {
                code: KeyCode::Char('v'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Single(Action::SwitchMode(Mode::Visual)),
                );
            }
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_waiting_command(stdout, editor, Action::CopyUnresolved);
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::Paste));
            }
            KeyEvent {
                code: KeyCode::Char('z'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_waiting_command(stdout, editor, Action::CenterUnresolved);
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
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveDown));
            }
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveUp));
            }
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveBackwards));
            }
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveForward));
            }
            KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::PopChar));
            }
            KeyEvent {
                code: KeyCode::Char('i'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Single(Action::SwitchMode(Mode::Insert)),
                );
                queue!(stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)?
            }
            KeyEvent {
                code: KeyCode::Char('A'),
                modifiers: KeyModifiers::SHIFT,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::AppendEndOfLine));
                queue!(stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)?
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(
                    stdout,
                    editor,
                    Motion::Single(Action::SwitchMode(Mode::Insert)),
                );
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveForward));
                queue!(stdout, crossterm::cursor::SetCursorStyle::BlinkingBar)?
            }
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveWord));
            }
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveBackWord));
            }
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveEndWord));
            }
            KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::WriteCurrentBuffer));
            }
            KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                if editor.mode == Mode::Visual {
                    self.handle_motions(stdout, editor, Motion::Single(Action::DeleteVisualMode));
                } else {
                    self.handle_waiting_command(stdout, editor, Action::DeleteUnresolved);
                }
            }
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                if editor.mode == Mode::Visual {
                    self.handle_motions(stdout, editor, Motion::Single(Action::ChangeVisualMode));
                } else {
                    self.handle_waiting_command(stdout, editor, Action::ChangeUnresolved);
                }
            }
            KeyEvent {
                code: KeyCode::Char('$'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            } => {
                self.handle_motions(stdout, editor, Motion::Single(Action::MoveEndOfLine));
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

    fn handle_waiting_command(&mut self, stdout: &mut impl Write, editor: &mut Editor, a: Action) {
        if editor.mode == Mode::Visual {
            // No motions like this in visual mode
            return;
        }
        if let Some(action) = &self.waiting_action {
            if a == action.1 {
                return self.handle_motions(stdout, editor, Motion::Single(Action::ActOnSelf));
            }
        }
        if let Some(repeater) = self.repeater {
            self.waiting_action = Some((Some(repeater), a));
        } else {
            self.waiting_action = Some((None, a));
        }
    }
}

impl Component for EditorBuffer {
    fn update_cursor(&mut self, editor: &mut Editor) -> (u16, u16) {
        let (editor_x, editor_y) = editor.cursor_pos;
        // let (client_x, client_y) = self.cursor_pos;
        let viewport_height = (self.viewport.height).saturating_sub(1);
        let viewport_width = (self.viewport.width).saturating_sub(1);
        if editor_y >= viewport_height * 3 / 4 + self.top_index {
            // We need to scroll down
            self.top_index += editor_y - (viewport_height * 3 / 4 + self.top_index);
        }
        if editor_y < self.top_index + viewport_height * 1 / 4 {
            // We need to scroll up
            self.top_index = self
                .top_index
                .saturating_sub(self.top_index + viewport_height * 1 / 4 - editor_y);
        }
        if editor_x >= viewport_width - self.left_offset + self.side_scroll {
            // We need to scroll sideways
            self.side_scroll += editor_x - (viewport_width + self.side_scroll - self.left_offset);
        }
        if editor_x < self.side_scroll + self.left_offset {
            // We need to scroll left
            self.side_scroll = self
                .side_scroll
                .saturating_sub((self.side_scroll).saturating_sub(editor_x));
        }
        //Essentially we need to check which char our cursor is on, and find out how much we should
        //shift our cursor based on how many \t were before it, since representations of \t on a
        //buffer level are just singular characters
        let curr_line = &editor.buffer.lines[editor_y];

        let take_amount = if editor.mode == Mode::Normal {
            editor_x + 1
        } else {
            editor_x
        };
        let shiftwidth = curr_line
            .chars()
            .skip(self.side_scroll)
            .take(take_amount)
            .enumerate()
            .fold(0, |acc: usize, c| {
                let (i, char) = c;
                if char == '\t' {
                    return acc
                        + Editor::get_spaces_till_next_tab((acc) + i + self.side_scroll, TABSTOP)
                        - 1;
                }
                acc
            });
        let x = (self.left_offset as u16 + editor_x as u16 + shiftwidth as u16)
            .saturating_sub(self.side_scroll as u16);
        let y = (editor_y - self.top_index) as u16;
        (x, y)
    }

    fn draw(&mut self, buffer: &mut RenderBuffer, editor: &mut Editor) {
        self.draw_line_numbers(buffer, editor);
        self.draw_lines(buffer, editor);
        self.elapsed_frames = self.elapsed_frames.saturating_add(1);
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
        stdout: &mut Box<dyn Write>,
        editor: &mut Editor,
        event: Event,
    ) -> anyhow::Result<()> {
        match event {
            Event::Key(ev) => match editor.mode {
                Mode::Normal => self.handle_normal_keys(&mut (*stdout), editor, ev)?,
                Mode::Insert => self.handle_insert_keys(&mut (*stdout), editor, ev)?,
                Mode::Visual => self.handle_normal_keys(&mut (*stdout), editor, ev)?,
            },
            _ => {}
        }
        Ok(())
    }
}
