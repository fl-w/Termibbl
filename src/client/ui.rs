mod canvas;
pub mod input;
pub mod room;
pub mod start;

use std::io::Stdout;

use crate::{
    message::ChatMessage,
    world::{Coord, DrawingWord, Game, Player, Username},
};

use crossterm::event::{KeyEvent, MouseEvent};
use tui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget},
    Frame,
};

use self::{
    canvas::{Palette, TermCanvas},
    input::{Cursor, InputText},
};

use Constraint::*;

pub type Action = Box<dyn Fn(&mut super::App)>;
pub type Backend = CrosstermBackend<Stdout>;

pub fn backend() -> Backend { CrosstermBackend::new(std::io::stdout()) }

#[macro_export]
macro_rules! do_nothing {
    () => {
        Box::new(|_| ())
    };
}

pub trait View {
    fn on_resize(&mut self, size: Coord) -> Action;
    fn on_key_event(&mut self, event: KeyEvent) -> Action;
    fn on_mouse_event(&mut self, event: MouseEvent) -> Action;
    fn draw(&mut self, frame: &mut Frame<Backend>);
}

pub trait ElementHolder {
    fn element_in<E: Element>(&self, coord: Coord) -> Option<&E>;
    fn element_in_mut<E: Element>(&mut self, coord: Coord) -> Option<&mut E>;
}

pub trait Element: Sized {
    fn on_resize(&mut self, bounds: Rect) -> Action;
    fn coord_within(&self, coord: Coord) -> bool;
    fn render(&self, area: Rect, buf: &mut Buffer);
    // fn render_stateful(&self, area: Rect, buf: &mut Buffer);
    fn as_widget(&self) -> ElementWidet<'_, Self> {
        ElementWidet {
            inner: Box::new(self),
        }
    }
}

pub struct ElementWidet<'a, E: Element> {
    inner: Box<&'a E>,
}

impl<'a, E> Widget for ElementWidet<'a, E>
where
    E: Element,
{
    fn render(self, area: Rect, buf: &mut Buffer) { self.inner.render(area, buf) }
}

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

pub struct CanvasWidget<'a> {
    canvas: &'a TermCanvas,
    palette: &'a Palette,
}

impl<'a> CanvasWidget<'a> {
    const PALETTE_HEIGHT: u16 = 2;
    pub fn new(canvas: &'a TermCanvas, palette: &'a Palette) -> CanvasWidget<'a> {
        CanvasWidget { canvas, palette }
    }
}

impl<'a, 'b> Widget for CanvasWidget<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let layout = Layout::default()
            .constraints([Percentage(100), Length(Self::PALETTE_HEIGHT)])
            .split(area);

        // draw palette
        let swatch_size = area.width / canvas::PALETTE.len() as u16;

        for (idx, col) in canvas::PALETTE.iter().enumerate() {
            for offset in 0..swatch_size {
                for y in 0..Self::PALETTE_HEIGHT - 1 {
                    buf.get_mut((swatch_size * idx as u16) + offset, y)
                        .set_bg((*col).into());
                }
            }
        }

        // draw canvas
        self.canvas.render(layout[0], buf);
    }
}

pub struct ChatWidget<'t> {
    messages: &'t [ChatMessage],
    input: &'t InputText,
}

impl<'t> ChatWidget<'t> {
    pub fn new(messages: &'t [ChatMessage], input: &'t InputText) -> ChatWidget<'t> {
        ChatWidget { messages, input }
    }
}

impl<'a, 'b> StatefulWidget for ChatWidget<'a> {
    type State = Cursor;

    fn render(
        self,
        area: tui::layout::Rect,
        buf: &mut tui::buffer::Buffer,
        cursor: &mut Self::State,
    ) {
        let mut chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Length(3), Length(area.height - 3)].as_ref())
            .split(area)
            .into_iter();

        let chat_messages: Vec<ListItem> = self
            .messages
            .iter()
            .rev()
            .map(|msg| {
                Span::styled(
                    format!("{}", msg),
                    if msg.is_system() {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                )
            })
            .map(ListItem::new)
            .collect();

        Paragraph::new(self.input.content())
            .block(Block::default().borders(Borders::ALL).title("Your message"))
            .render(chunks.next().unwrap(), buf);

        <List as Widget>::render(
            List::new(chat_messages).block(Block::default().borders(Borders::LEFT).title("Chat")),
            chunks.next().unwrap(),
            buf,
        );

        if self.input.has_focus() {
            cursor.set(area.x + self.input.cursor() as u16, area.y);
        }
    }
}

pub struct SkribblStateWidget<'t> {
    game: &'t Game,
    players: &'t [Player],
    username: &'t Username,
    remaining_time: u32,
}

impl<'t> SkribblStateWidget<'t> {
    pub fn new(
        username: &'t Username,
        players: &'t [Player],
        game: &'t Game,
        // remaining_time: u32,
    ) -> SkribblStateWidget<'t> {
        SkribblStateWidget {
            game,
            players,
            username,
            remaining_time: 0,
        }
    }
}

impl<'a, 'b> Widget for SkribblStateWidget<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([Constraint::Length(1), Constraint::Percentage(100)].as_ref())
            .split(area);

        let (hint, style) = match &self.game.turn.word {
            DrawingWord::Guess {
                hints,
                word_len,
                who,
            } => {
                // get the placeholder chars for the current word, with the revealed characters revealed.
                let hint = (0..*word_len)
                    .map(|ref idx| hints.get(idx).cloned().unwrap_or('?'))
                    .collect::<String>();

                (format!("{} drawing {}", who.name(), hint), Style::default())
            }
            DrawingWord::Draw(word) => (format!("Draw {}", word), Style::default().bg(Color::Red)),
        };

        Paragraph::new(Span::styled(hint, style)).render(chunks[0], buf);

        let player_list: Vec<ListItem> = self
            .players
            .iter()
            .map(|ref player| {
                let username = &player.name;
                let is_drawing = match &self.game.turn.word {
                    DrawingWord::Draw(_) => username == self.username,
                    DrawingWord::Guess { who, .. } => username == who,
                };

                Span::styled(
                    format!("{}: {}", username, player.score,),
                    if is_drawing {
                        Style::default().bg(tui::style::Color::Cyan)
                    } else if player.solved_current_round {
                        Style::default().fg(tui::style::Color::Green)
                    } else {
                        Style::default()
                    },
                )
            })
            .map(ListItem::new)
            .collect();

        <List as Widget>::render(
            List::new(player_list).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Players [time: {}]", self.remaining_time)),
            ),
            chunks[1],
            buf,
        );
    }
}
