use tui::widgets::Paragraph;

use crate::{
    client::ui::{centered_area, View},
    do_nothing,
};

#[derive(Default)]
pub struct Lobby;

impl View for Lobby {
    fn on_resize(&mut self, size: crate::data::Coord) {}

    fn on_key_event(&mut self, event: crossterm::event::KeyEvent) -> crate::client::ui::Action {
        do_nothing!()
    }

    fn on_mouse_event(&mut self, event: crossterm::event::MouseEvent) -> crate::client::ui::Action {
        do_nothing!()
    }

    fn draw(&self, frame: &mut tui::Frame<crate::client::ui::Backend>, app: &crate::client::App) {
        frame.render_widget(
            Paragraph::new("Waiting for more players"),
            centered_area((24, 1), frame.size()),
        )
    }
}
