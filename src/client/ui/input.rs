use tui::layout::Rect;

use crossterm::event::KeyCode;
#[derive(Default, Debug)]
pub struct Cursor(Option<(u16, u16)>);

impl Cursor {
    pub fn set(&mut self, x: u16, y: u16) { self.0.replace((x, y)); }
    pub fn take(&mut self) -> Option<(u16, u16)> { self.0.take() }
}

#[derive(Default, Debug, Clone)]
pub struct InputText {
    content: String,
    cursor_position: usize,
    in_focus: bool,
}

impl InputText {
    pub fn on_key_event(&mut self, code: KeyCode) {
        if self.in_focus {
            match code {
                KeyCode::Home => self.cursor_position = 0,
                KeyCode::End => self.cursor_position = self.content.len(),
                KeyCode::Char(ch) => {
                    self.content.insert(self.cursor_position, ch);
                    self.cursor_position += 1;
                }
                KeyCode::Backspace => {
                    if self.cursor_position > 0 {
                        self.cursor_position -= 1;
                        self.content.remove(self.cursor_position);
                    }
                }
                KeyCode::Left => {
                    if self.cursor_position > 0 {
                        self.cursor_position -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.cursor_position < self.content.len() {
                        self.cursor_position += 1;
                    }
                }
                _ => (),
            }
        };
    }

    pub fn focus(&mut self, focus: bool) { self.in_focus = focus; }

    pub fn content(&self) -> &str { &self.content }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor_position = self.content.len();
    }

    pub fn cursor(&self) -> usize { self.cursor_position }

    pub fn has_focus(&self) -> bool { self.in_focus }

    pub fn drain(&mut self) -> String {
        let content = self.content.drain(..).collect();
        self.cursor_position = 0;

        content
    }
}

/// helper function to create a centered rect using up
/// certain percentage of the available rect `r`
pub fn centered_area(dimension: (u16, u16), rect: Rect) -> Rect {
    let width_diff = rect.width as i16 - dimension.0 as i16;
    let height_diff = rect.height as i16 - dimension.1 as i16;
    let x = if width_diff > 0 {
        rect.x + width_diff as u16 / 2
    } else {
        0
    };
    let y = if height_diff > 0 {
        rect.y + height_diff as u16 / 2
    } else {
        0
    };
    let width = if rect.width > dimension.0 {
        dimension.0
    } else {
        rect.width
    };
    let height = if rect.height > dimension.1 {
        dimension.1
    } else {
        rect.height
    };

    Rect::new(x, y, width, height)
}
