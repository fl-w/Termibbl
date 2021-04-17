mod lobby;
mod skribbl;

use std::collections::HashMap;

use crossterm::event;
use event::{MouseButton, MouseEventKind};
use tui::{
    backend::Backend,
    layout::{
        Constraint::{Length, Percentage},
        Direction, Layout,
    },
    style::Style,
    widgets::{Block, Borders},
    Frame,
};

use crate::{
    do_nothing,
    message::{self, ChatMessage},
    world::{Coord, Draw, DrawingWord, Game, Player, RoomState, Username},
};

use self::{lobby::Lobby, skribbl::Skribbl};

use super::{
    canvas::{self, PaintTool},
    input::Cursor,
    Action, BlockWidget, CanvasWidget, ChatWidget, Element, ElementHolder, SkribblStateWidget,
    View,
};

pub struct Room {
    pub username: Username,
    pub state: RoomState<Skribbl>,
    pub player_list: Vec<Player>,
    pub lobby: Lobby,
}

impl Room {
    pub fn new(username: Username, player_list: Vec<Player>, state: RoomState<Game>) -> Self {
        let mut new = Self {
            username,
            state: RoomState::Lobby,
            player_list,
            lobby: Lobby::default(),
        };

        new.update_state(state);

        new
    }

    pub fn update_state(&mut self, mut state: RoomState<Game>) {
        let canvas = &mut self.canvas;

        canvas.resize(state.dimensions().unwrap_or_default());
        if let Some(state_canvas) = state.canvas_mut() {
            state_canvas
                .iter()
                .fold(HashMap::new(), |mut points_by_color, (point, col)| {
                    points_by_color
                        .entry(*col)
                        .or_insert_with(Vec::new)
                        .push(*point);
                    points_by_color
                })
                .into_iter()
                .for_each(|(col, points)| canvas.paint(points.as_slice(), col));

            state_canvas.clear();
        }

        self.state = state;
    }

    pub fn username(&self) -> &Username { &self.username }

    pub fn can_draw(&self) -> bool {
        self.state
            .world()
            .map(|game_info| matches!(&game_info.turn.word, DrawingWord::Draw(_)))
            .unwrap_or(false)
    }
}

impl ElementHolder for Room {
    fn element_in<E: Element>(&self, coord: Coord) -> Option<&E> { todo!() }
    fn element_in_mut<E: Element>(&mut self, coord: Coord) -> Option<&mut E> { todo!() }
}

impl View for Room {
    fn on_resize(&mut self, _size: Coord) -> Action {
        // match &self.state {
        //     GameState::Lobby => {}
        //     GameState::FreeDraw(canvas) => {}
        //     GameState::Drawing(game_state) => {
        //         let canvas = &self.canvas;
        //         let canvas_width = canvas.width as u16;
        //         let canvas_height = canvas.height as u16;

        //         // split tui window
        //         let main_chunks = Layout::default()
        //             .direction(Direction::Horizontal)
        //             .margin(0)
        //             .constraints(
        //                 [
        //                     Length(canvas_width),
        //                     Length(if size.width < canvas_width {
        //                         size.width
        //                     } else {
        //                         size.width - canvas_width
        //                     }),
        //                 ]
        //                 .as_ref(),
        //             )
        //             .split(size);

        //         let canvas_area = main_chunks[0];

        //         self.canvas

        //         let game_state_height = game.player_list.len() as u16;
        //         let sidebar_chunks = Layout::default()
        //             .direction(Direction::Vertical)
        //             .margin(0)
        //             .constraints([Length(game_state_height + 3), Percentage(100)].as_ref())
        //             .split(main_chunks[1]);

        //         if let Some(skribbl_state) = game.state.world() {
        //             let skribbl_widget =
        //                 SkribblStateWidget::new(game.username(), &game.player_list, skribbl_state);

        //             f.render_widget(
        //                 BlockWidget::new()
        //                     .block(Block::default().borders(Borders::NONE))
        //                     .widget(skribbl_widget),
        //                 sidebar_chunks[0],
        //             );
        //         }
        //     }
        // };
        do_nothing!()
    }

    // let chat_widget = self.chat.as_widget();
    // let canvas_widget = self.canvas.as_widget();
    // let mut cursor = Cursor::default();

    // // BlockWidget::new().widget(ChatWidget::new(

    // // self.chat.messages.as_slice(),
    // // &self.chat.input,
    // // ));

    // // let canvas_widget = CanvasWidget::new(canvas, &game.palette);
    // // BlockWidget::new().widget(canvas_widget).block(
    // //     Block::default()
    // //         .borders(Borders::ALL)
    // //         .style(Style::default().fg(game.palette.selected_color.into())),
    // // ),

    // f.render_widget(canvas_widget, self.canvas.area());
    // f.render_stateful_widget(chat_widget, self.sidebar_chunks[1], &mut cursor);

    // if let Some((x, y)) = cursor.take() {
    //     f.set_cursor(x, y)
    // }

    fn on_key_event(&mut self, event: event::KeyEvent) -> Action {
        let input = &mut self.chat.input;

        match event.code {
            event::KeyCode::Enter => {
                if input.has_focus() && !input.content().is_empty() {
                    let chat_msg = input.drain();
                    let username = self.username.clone();

                    let message = message::ToServer::Chat(ChatMessage::User(username, chat_msg));

                    Box::new(move |app| app.server_mut().send_message(message.clone()))
                } else {
                    do_nothing!()
                }
            }

            event::KeyCode::Delete => {
                if self.can_draw() {
                    self.canvas.clear();
                    Box::new(|app| {
                        app.server_mut()
                            .send_message(message::ToServer::Draw(Draw::Clear))
                    })
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

    fn on_mouse_event(&mut self, event: event::MouseEvent) -> Action {
        let (x, y) = (event.column, event.row);
        let can_draw = self.can_draw();

        match &mut self.state {
            RoomState::Playing(_) => {
                let canvas = if can_draw {
                    &mut self.canvas
                } else {
                    return do_nothing!();
                };

                match event.kind {
                    MouseEventKind::Down(MouseButton::Right) => {
                        // select fill tool with right mouse key
                        self.palette.paint_tool = PaintTool::Fill;
                    }
                    MouseEventKind::Down(MouseButton::Left)
                    | MouseEventKind::Drag(MouseButton::Left) => {
                        if y == 0 {
                            let swatch_size = crossterm::terminal::size().unwrap().0
                                / canvas::PALETTE.len() as u16;
                            let selected_color_index = x / swatch_size;

                            if let Some(color) = canvas::PALETTE.get(selected_color_index as usize)
                            {
                                self.palette.selected_color = *color;
                            }

                            return do_nothing!();
                        }
                    }

                    MouseEventKind::Up(_) => {
                        self.palette.last_mouse_pos = None;
                        return do_nothing!();
                    }

                    _ => return do_nothing!(),
                };

                let palette = &mut self.palette;
                let mouse_pos = (x as isize, y as isize);
                let old_mouse_pos = palette.last_mouse_pos.replace(mouse_pos);

                let points =
                    line_drawing::Bresenham::new(old_mouse_pos.unwrap_or(mouse_pos), mouse_pos)
                        // .skip(if has_prev_point { 1 } else { 0 })
                        .map(|(x, y)| (x as u16, y as u16))
                        .collect::<Vec<_>>();

                // apply draw to canvas before sending to server.
                canvas.paint(points.as_slice(), palette.selected_color);

                let color = palette.selected_color;
                return Box::new(move |app| {
                    app.server_mut()
                        .send_message(message::ToServer::Draw(Draw::Paint {
                            color,
                            points: points.clone(),
                        }))
                });
            }
            RoomState::FreeDraw => {}
            RoomState::Lobby => {}
            RoomState::Waiting => {}
        };

        do_nothing!()
    }

    fn draw<B: Backend>(app: &mut crate::client::App, frame: &mut Frame<B>) { todo!() }
}

pub fn draw_game_view<B>(f: &mut Frame<B>, game_room: &Room)
where
    B: Backend,
{
    let size = f.size();
    match &game_room.state {
        RoomState::Lobby => {}
        RoomState::Waiting => {}
        RoomState::FreeDraw => {}
        RoomState::Playing(game_state) => {
            let canvas = &game_room.canvas;
            let canvas_width = canvas.width as u16;
            let canvas_height = canvas.height as u16;

            // split tui window
            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints(
                    [
                        Length(canvas_width),
                        Length(if size.width < canvas_width {
                            size.width
                        } else {
                            size.width - canvas_width
                        }),
                    ]
                    .as_ref(),
                )
                .split(size);

            // render canvas
            let canvas_widget = CanvasWidget::new(canvas, &game_room.palette);
            let canvas_area = main_chunks[0];
            f.render_widget(
                BlockWidget::new().widget(canvas_widget).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(game_room.palette.selected_color.into())),
                ),
                canvas_area,
            );

            let game_state_height = game_room.player_list.len() as u16;
            let sidebar_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Length(game_state_height + 3), Percentage(100)].as_ref())
                .split(main_chunks[1]);

            if let Some(skribbl_state) = game_room.state.world() {
                let skribbl_widget = SkribblStateWidget::new(
                    game_room.username(),
                    &game_room.player_list,
                    skribbl_state,
                );

                f.render_widget(
                    BlockWidget::new()
                        .block(Block::default().borders(Borders::NONE))
                        .widget(skribbl_widget),
                    sidebar_chunks[0],
                );
            }

            let chat_widget = BlockWidget::new().widget(ChatWidget::new(
                game_room.chat.messages.as_slice(),
                &game_room.chat.input,
            ));

            let mut cursor = Cursor::default();

            f.render_stateful_widget(chat_widget, sidebar_chunks[1], &mut cursor);

            if let Some((x, y)) = cursor.take() {
                f.set_cursor(x, y)
            }
        }
    }
}
