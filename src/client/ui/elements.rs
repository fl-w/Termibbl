use crate::data::Coord;

use tui::{buffer::Buffer, layout::Rect, widgets::Widget};

pub trait Element {
    fn resize(&mut self, bounds: Rect);
    fn bounds(&self) -> Rect;
    fn render(&self, buf: &mut Buffer);
    fn coord_within(&self, (x, y): Coord) -> bool {
        self.bounds().intersects(Rect {
            x,
            y,
            width: 0,
            height: 0,
        })
    }
    fn as_widget(&self) -> ElementWidet<'_, Self>
    where
        Self: Sized,
    {
        ElementWidet { inner: self }
    }
}

pub struct ElementWidet<'a, E: Element> {
    inner: &'a E,
}

impl<'a, E> Widget for ElementWidet<'a, E>
where
    E: Element,
{
    fn render(self, _area: Rect, buf: &mut Buffer) { self.inner.render(buf) }
}
