use crossterm::event::KeyCode;
use tui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Paragraph, StatefulWidget, Widget},
};

#[derive(Default, Debug)]
pub struct Cursor(Option<(u16, u16)>);

impl Cursor {
    pub fn set(&mut self, x: u16, y: u16) { self.0.replace((x, y)); }
    pub fn take(&mut self) -> Option<(u16, u16)> { self.0.take() }
}

#[derive(Default, Debug, Clone)]
pub struct InputField {
    content: String,
    cursor_position: usize,
    in_focus: bool,
}

impl InputField {
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

    pub fn is_empty(&self) -> bool { self.content.is_empty() }

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

pub type Hint = (&'static str, Color);

pub struct InputWidget<'a> {
    label: &'a str,
    input: &'a InputField,
    hint: Hint,
}

impl<'a> InputWidget<'a> {
    pub fn new(label: &'a str, hint: Hint, input: &'a InputField) -> Self {
        Self { label, input, hint }
    }
}

impl StatefulWidget for InputWidget<'_> {
    type State = Cursor;

    fn render(self, area: Rect, buf: &mut Buffer, cursor: &mut Cursor) {
        let label = Spans::from(vec![
            Span::raw(self.label),
            Span::styled(
                self.input.content(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]);

        // draw inputbox label
        Paragraph::new(label)
            .alignment(Alignment::Left)
            .render(area, buf);

        // draw hint
        let (message, hint_color) = self.hint;
        let hint = Span::styled(message, Style::default().fg(hint_color));

        Paragraph::new(hint)
            .alignment(Alignment::Right)
            .render(area, buf);

        if self.input.has_focus() {
            cursor.set(
                area.x + (self.label.len() + self.input.cursor()) as u16,
                area.y,
            );
        }
    }
}
