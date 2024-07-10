use crate::{
    editor::{
        buffer::{RenderBuffer, Viewport},
        Editor, EditorStatus, Mode, TABSTOP,
    },
    styles::{default_line_number_style, default_text_style, mode_style},
};

pub fn resize_viewport(viewport: &Viewport, w: usize, h: usize) -> Viewport {
    Viewport {
        pos: viewport.pos,
        width: w,
        height: h,
    }
}

//Perfect place for a Macro to generate new methods for me, split the resize stuff into a seperate
//trait and have all my components derive it, boom done, for now we do it by hand
pub trait Component {
    fn update_cursor(&mut self, editor: &mut Editor) -> (u16, u16);
    fn draw(&mut self, buffer: &mut RenderBuffer, editor: &mut Editor);
    fn get_viewport(&self) -> &Viewport;
    fn resize(&mut self, w: usize, h: usize);
    fn set_resize_callback(&mut self, c: Box<dyn Fn(usize, usize) -> Viewport>);
}

pub struct EditorBuffer {
    top_index: usize,
    left_offset: usize, // For line numbers,
    side_scroll: usize,
    viewport: Viewport,
    resize_callback: Box<dyn Fn(usize, usize) -> Viewport>,
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
        }
    }
    pub fn draw_lines(&mut self, render_buffer: &mut RenderBuffer, editor: &mut Editor) {
        for (i, line) in editor.buffer.lines.iter().skip(self.top_index).enumerate() {
            if i >= self.viewport.height as usize {
                break;
            }

            // Transform \t into appropriate amount of spaces, using size instead of len() to avoid
            // counting string length everytime
            let mut s = String::new();
            let mut size = 0;
            for c in line.chars() {
                if c == '\t' {
                    for _ in 0..Editor::get_spaces_till_next_tab(size, TABSTOP) {
                        s.push(' ');
                        size += 1;
                    }
                } else {
                    s.push(c);
                    size += 1;
                }
            }
            // We now skip the string its appropriate left_offset, we always have to calculate it
            // like this to avoid cases because of tabs
            let skipped_string: String = s.chars().skip(self.side_scroll).collect::<String>();
            render_buffer.put_str(
                &skipped_string,
                (self.left_offset, i),
                default_text_style(),
                &self.viewport,
            );
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

            render_buffer.put_str(&padded, (0, i), default_line_number_style(), &self.viewport);
        }
    }
}

impl Component for EditorBuffer {
    fn update_cursor(&mut self, editor: &mut Editor) -> (u16, u16) {
        let (editor_x, editor_y) = editor.cursor_pos;
        // let (client_x, client_y) = self.cursor_pos;
        let viewport_height = (self.viewport.height).saturating_sub(1);
        let viewport_width = (self.viewport.width).saturating_sub(1);
        if editor_y >= viewport_height + self.top_index {
            // We need to scroll down
            self.top_index += editor_y - (viewport_height + self.top_index);
        }
        if editor_y < self.top_index {
            // We need to scroll up
            self.top_index -= self.top_index - editor_y;
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
    fn update_cursor(&mut self, _editor: &mut Editor) -> (u16, u16) {
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
            default_text_style(),
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
}

pub struct MessagesComponent {
    viewport: Viewport,
    resize_callback: Box<dyn Fn(usize, usize) -> Viewport>
}

impl MessagesComponent {
   pub fn new(viewport: Viewport, resize_callback: Box<dyn Fn(usize, usize) -> Viewport>) -> MessagesComponent{
        MessagesComponent{
            viewport,
            resize_callback
        }
    }
}

impl Component for MessagesComponent{

    fn get_viewport(&self) -> &Viewport {
        &self.viewport
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.viewport = (self.resize_callback)(w, h);
    }

    fn set_resize_callback(&mut self, c: Box<dyn Fn(usize, usize) -> Viewport>) {
        self.resize_callback = c;
    }

    fn update_cursor(&mut self, _editor: &mut Editor) -> (u16, u16) {
        (0,0)
    }

    fn draw(&mut self, buffer: &mut RenderBuffer, editor: &mut Editor) {
        buffer.put_str(
            &editor.message,
            (0, 0),
            default_text_style(),
            &self.viewport,
        );
    }
}

