use crossterm::event::{self, KeyEvent, MouseButton, MouseEventKind};
use tui::{
    layout::{Constraint::*, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, StatefulWidget, Widget},
    Frame,
};

use crate::{
    client::{
        ui::{
            centered_area,
            elements::Element,
            widgets::{
                block::BlockWidget,
                canvas::{PaintTool, Palette, TermCanvas, PALETTE},
                chat::Chat,
                input::Cursor,
            },
            Action, Backend, View,
        },
        App,
    },
    data::{Coord, GameInfo, GameOpts, GameState, PlayerData, TurnPhase, UserId, WordHint},
    do_nothing,
    message::{ChatMessage, Draw, ToServer},
};

pub struct Game {
    pub info: GameInfo,
    pub palette: Palette,
    pub canvas: TermCanvas,
    pub chat: Chat,
    sidebar_area: Rect,
    pub player_id: UserId,
}

impl Game {
    pub fn new(info: GameInfo, game_opts: &GameOpts, player_id: UserId) -> Self {
        Self {
            info,
            canvas: TermCanvas::new(game_opts.dimensions),
            palette: Palette::new(PALETTE),
            chat: Chat::new(),
            sidebar_area: Rect::default(),
            player_id,
        }
    }

    pub fn update_state(&mut self, state: GameState) {
        self.info.state = state;
        match &mut self.info.state {
            GameState::RoundStart(_) => {}
            GameState::Playing(_) => {}
            GameState::Finish => self.info.players.sort_by(|a, b| a.score.cmp(&b.score)),
        }
    }

    pub fn am_i_drawing(&self) -> bool {
        self.info
            .state
            .as_turn()
            .map(|turn| turn.who_is_drawing == self.player_id)
            .unwrap_or(false)
    }
}

impl View for Game {
    fn on_key_event(&mut self, event: KeyEvent) -> Action {
        let input = &mut self.chat.input;

        match event.code {
            event::KeyCode::Enter => {
                if input.has_focus() && !input.content().is_empty() {
                    let chat_msg = input.drain();

                    Box::new(move |app: &mut App| {
                        let username = app.username().cloned().unwrap();
                        let message = ToServer::Chat(ChatMessage::User(username, chat_msg));

                        app.server().send_message(message);
                    })
                } else {
                    do_nothing!()
                }
            }

            event::KeyCode::Delete => {
                if self.am_i_drawing() {
                    self.canvas.clear();
                    Box::new(|app| app.server().send_message(ToServer::Draw(Draw::Clear)))
                } else {
                    do_nothing!()
                }
            }

            _ => {
                input.on_key_event(event.code);
                do_nothing!()
            }
        }
    }

    fn on_mouse_event(&mut self, event: crossterm::event::MouseEvent) -> Action {
        let (x, y) = (event.column, event.row);
        let i_am_drawing = self.am_i_drawing();
        let palette = &mut self.palette;
        let canvas = &mut self.canvas;

        if palette.coord_within((x, y)) {
            match event.kind {
                MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left) => palette.select_color_on_coord((x, y)),

                _ => (),
            }
        }

        // if clicking on the canvas
        if i_am_drawing && canvas.coord_within((x, y)) {
            match event.kind {
                MouseEventKind::Down(MouseButton::Right) => {
                    // select fill tool with right mouse key
                    palette.paint_tool = PaintTool::Fill;
                }
                MouseEventKind::Down(MouseButton::Left)
                | MouseEventKind::Drag(MouseButton::Left) => {
                    let color = palette.selected_color();
                    let draw = canvas.draw((x, y), color);

                    return Box::new(move |app| app.server().send_message(ToServer::Draw(draw)));
                }

                MouseEventKind::Up(_) => canvas.reset_mouse(),

                _ => (),
            };
        }

        do_nothing!()
    }

    fn on_resize(&mut self, size: Coord) {
        // split tui window
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints([Length(size.0 - 30), Length(30)].as_ref())
            .split(Rect {
                x: 0,
                y: 0,
                width: size.0,
                height: size.1,
            });

        // split canvas
        let canvas_chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Length(1), // word to draw
                if !self.am_i_drawing() || self.palette.is_hidden() {
                    Length(0)
                } else {
                    Length(3)
                }, // palette height
                Percentage(100),
            ])
            .split(main_chunks[0]);

        self.palette.resize(canvas_chunks[1]);
        // bound drawing word - make same width as canvas area,
        self.canvas.resize(canvas_chunks[2]);

        self.sidebar_area = main_chunks[1];
    }

    fn draw(&self, f: &mut Frame<Backend>, _: &App) {
        // render canvas
        f.render_widget(
            BlockWidget::new().widget(self.canvas.as_widget()).block(
                Block::default()
                    .border_type(BorderType::Rounded)
                    .style(Style::default().fg(self.palette.selected_color().into())),
            ),
            self.canvas.bounds(),
        );

        // render palette
        f.render_widget(
            BlockWidget::new().widget(self.palette.as_widget()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            ),
            self.palette.bounds(),
        );

        match &self.info.state {
            GameState::RoundStart(round) => {
                let round_start_text = format!("Round {}...", round);
                let length = round_start_text.len() as u16;
                f.render_widget(
                    Paragraph::new(round_start_text),
                    centered_area((length, 1), self.canvas.bounds()),
                );
            }
            GameState::Playing(turn) => {
                let who_is_drawing = self
                    .info
                    .get_player(turn.who_is_drawing)
                    .map(|pl| pl.name.to_string())
                    .unwrap_or_else(String::new);

                match &turn.phase {
                    TurnPhase::ChoosingWord(_) => {
                        // todo:
                    }
                    TurnPhase::Drawing(hint) => {
                        // render word to draw
                        let (mut word, style) = match hint {
                            WordHint::Hint { hints, word_len } => {
                                // get the placeholder chars for the current word, with the revealed characters revealed.
                                let hint = (0..*word_len)
                                    .map(|ref idx| hints.get(idx).cloned().unwrap_or('_'))
                                    .collect::<String>();

                                (
                                    format!("{} is drawing {}", who_is_drawing, hint),
                                    Style::default(),
                                )
                            }
                            WordHint::Draw(word) => {
                                (format!("Draw {}", word), Style::default().bg(Color::Red))
                            }
                        };
                        word.truncate(u16::MAX.into());

                        let width = word.len() as u16;
                        let x = std::cmp::max(
                            0,
                            (self.canvas.bounds().width as isize - width as isize) / 2,
                        ) as u16;

                        let word_rect = Rect {
                            x,
                            y: 0,
                            width,
                            height: 1,
                        };

                        f.render_widget(Paragraph::new(Span::styled(word, style)), word_rect);
                    }
                    TurnPhase::RevealWord {
                        word,
                        scores,
                        timed_out,
                    } => {
                        // todo reveal word
                    }
                };
            }
            GameState::Finish => {
                todo!()
            }
        }

        // now render sidebar -----------
        let mut cursor = Cursor::default();
        f.render_stateful_widget(
            BlockWidget::new().widget(SidebarWidget {
                players: &self.info.players,
                chat: &self.chat,
            }),
            self.sidebar_area,
            &mut cursor,
        );

        if let Some((x, y)) = cursor.take() {
            f.set_cursor(x, y)
        }
    }
}

struct SidebarWidget<'a> {
    players: &'a Vec<PlayerData>,
    chat: &'a Chat,
}

impl<'a> StatefulWidget for SidebarWidget<'a> {
    type State = Cursor;
    fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, cursor: &mut Self::State) {
        let player_list_height = self.players.len() as u16 + 2;
        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Length(area.height - player_list_height),
                    Length(player_list_height), // height is num of players + 2 borders
                ]
                .as_ref(),
            )
            .split(area);

        // render chat
        ChatWidget { chat: &self.chat }.render(sidebar_chunks[0], buf, cursor);

        // render player list
        let player_list: Vec<ListItem> = self
            .players
            .iter()
            .map(|ref player| {
                let username = &player.name;
                let is_drawing = false; // todo

                Span::styled(
                    format!("{}: {}", username, player.score,),
                    if is_drawing {
                        Style::default().bg(tui::style::Color::Cyan)
                    } else if player.solved_current_round() {
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
                    .border_type(BorderType::Double)
                    .borders(Borders::ALL)
                    .title("Players"),
            ),
            sidebar_chunks[1],
            buf,
        );
    }
}

struct ChatWidget<'a> {
    chat: &'a Chat,
}

impl<'a> StatefulWidget for ChatWidget<'a> {
    type State = Cursor;

    fn render(self, area: Rect, buf: &mut tui::buffer::Buffer, state: &mut Self::State) {
        let mut chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Length(3), // input field
                    Length(area.height - 3),
                ]
                .as_ref(),
            )
            .split(area)
            .into_iter();

        let chat_messages: Vec<ListItem> = self
            .chat
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

        Paragraph::new(self.chat.input.content())
            .block(Block::default().borders(Borders::ALL).title("Your message"))
            .render(chunks.next().unwrap(), buf);

        <List as Widget>::render(
            List::new(chat_messages).start_corner(tui::layout::Corner::TopRight),
            chunks.next().unwrap(),
            buf,
        );
        // .block(Block::default().borders(Borders::ALL).title("Chat")),
        if self.chat.input.has_focus() {
            state.set(area.x + 1 + self.chat.input.cursor() as u16, area.y + 1);
        }
    }
}
