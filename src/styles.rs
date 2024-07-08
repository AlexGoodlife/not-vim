use crossterm::style::{Attribute, Attributes, Color, ContentStyle};

use crate::editor::Mode;

pub const BLACK: Color = Color::Rgb{r:18,b:18,g:18};

pub fn default_text_style() -> ContentStyle {
    let attr = Attributes::default();
    // attr.set(Attribute::Reset);
    ContentStyle {
        foreground_color: Some(Color::Rgb {
            r: 215,
            g: 215,
            b: 215,
        }),
        background_color: Some(BLACK),
        underline_color: None,
        attributes: attr,
    }
}

pub fn default_line_number_style() -> ContentStyle {
    let attr = Attributes::default();
    // attr.set(Attribute::Reset);
    ContentStyle {
        foreground_color: Some(Color::Rgb {
            r: 50,
            g: 50,
            b: 50,
        }),
        background_color: Some(BLACK),
        underline_color: None,
        attributes: attr,
    }
}

pub fn gutter_style(mode: &Mode) -> ContentStyle {
    let mut attr = Attributes::default();
    // attr.set(Attribute::Reset);
    attr.set(Attribute::Bold);
    let color = match mode {
        Mode::Normal => Some(Color::Rgb {
            r: 100,
            g: 149,
            b: 171,
        }),
        Mode::Insert => Some(Color::Rgb {
            r: 0,
            g: 163,
            b: 108,
        }),
    };
    ContentStyle {
        foreground_color: color,
        background_color: color,
        underline_color: None,
        attributes: attr,
    }
}

pub fn mode_style(mode: &Mode) -> ContentStyle {
    let mut attr = Attributes::default();
    attr.set(Attribute::Bold);
    let color = match mode {
        Mode::Normal => Some(Color::Rgb {
            r: 100,
            g: 149,
            b: 171,
        }),
        Mode::Insert => Some(Color::Rgb {
            r: 0,
            g: 163,
            b: 108,
        }),
    };
    ContentStyle {
        foreground_color: Some(Color::Rgb {
            r: (0),
            g: (0),
            b: (0),
        }),
        background_color: color,
        underline_color: None,
        attributes: attr,
    }
}
