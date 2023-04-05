use tui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, StatefulWidget, Widget},
};

#[derive(Default)]
pub struct BlockWidget<'a, T> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    widget: Option<T>,
}

impl<'a, T> BlockWidget<'a, T> {
    pub fn new() -> Self {
        Self {
            block: None,
            widget: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> BlockWidget<'a, T> {
        self.block = Some(block);
        self
    }

    pub fn widget(mut self, widget: T) -> BlockWidget<'a, T> {
        self.widget = Some(widget);
        self
    }

    fn widget_area(&mut self, area: Rect, buf: &mut Buffer) -> Rect {
        match self.block.take() {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        }
    }
}

impl<'a, T> Widget for BlockWidget<'a, T>
where
    T: Widget,
{
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let widget_area = self.widget_area(area, buf);

        if let Some(widget) = self.widget.take() {
            widget.render(widget_area, buf)
        }
    }
}

impl<'a, T> StatefulWidget for BlockWidget<'a, T>
where
    T: StatefulWidget,
{
    type State = T::State;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let widget_area = self.widget_area(area, buf);

        if let Some(widget) = self.widget.take() {
            widget.render(widget_area, buf, state)
        }
    }
}
