mod elements;
mod room;
mod start;
pub mod widgets;

use crossterm::event::{KeyEvent, MouseEvent};
use tui::{backend::CrosstermBackend, layout::Rect, Frame};

use crate::data::Coord;

pub use self::{room::Room, start::StartMenu};

use super::App;

pub type Backend = CrosstermBackend<std::io::Stdout>;

pub fn backend() -> Backend { CrosstermBackend::new(std::io::stdout()) }

pub type Action = Box<dyn FnOnce(&mut App)>;

pub trait View {
    fn on_key_event(&mut self, event: KeyEvent) -> Action;
    fn on_mouse_event(&mut self, event: MouseEvent) -> Action;
    fn on_resize(&mut self, size: Coord);
    fn draw(&self, frame: &mut Frame<Backend>, app: &App);
}

#[macro_export]
macro_rules! do_nothing {
    () => {
        Box::new(|_| ())
    };
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
