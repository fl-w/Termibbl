use std::{net::SocketAddr, time::Duration};

use crossterm::event::{Event as InputEvent, KeyCode, KeyEvent, KeyModifiers};

use tui::Terminal;

use crate::events::{EventQueue, EventSender};

use super::{
    error::Result,
    net::{AppServer, ConnectionStatus, NetEvent},
    ui::{self, room::Room, start::StartMenu, View},
    CliOpts, Event,
};

// forces 5 frames per second
const MIN_FRAME_DURATION: f32 = 1.0 / 30.0;

enum State {
    Start(StartMenu),
    InGameRoom(Room),
}

pub struct App {
    event_queue: EventQueue<Event>,
    state: State,
    server: AppServer,
    should_exit: bool,
    forced_refresh_rate: Duration,
}

impl App {
    pub fn from_args(args: CliOpts) -> App {
        App {
            event_queue: EventQueue::default(),
            state: State::Start(StartMenu::new(args.host, args.username)),
            server: AppServer::default(),
            should_exit: false,
            forced_refresh_rate: Duration::from_secs_f32(MIN_FRAME_DURATION),
        }
    }

    pub fn server(&self) -> &AppServer { &self.server }

    pub fn server_mut(&mut self) -> &mut AppServer { &mut self.server }

    pub fn exit(&mut self) { self.should_exit = true; }

    pub fn sender(&self) -> &EventSender<Event> { self.event_queue.sender() }

    pub fn reset_connection_state(&mut self) {
        if !self.server.is_connected() {
            self.server.set_status(ConnectionStatus::NotConnected);
        }
    }

    pub fn get_current_view(&mut self) -> &mut dyn View {
        match &mut self.state {
            State::Start(start_menu) => start_menu,
            State::InGameRoom(room) => room,
        }
    }

    fn connect_to_server(&mut self, addr: SocketAddr) {
        self.server.connect(addr, self.event_queue.sender().clone());
    }

    async fn handle_net_event(&mut self, event: NetEvent) -> Result<()> {
        match event {
            NetEvent::Connected(session) => {
                self.server.set_session(session).await?;
            }

            NetEvent::Status(status) => {
                let addr = self.server.addr();
                self.server.set_status(status);

                let is_connected = self.server.is_connected();
                match &mut self.state {
                    State::InGameRoom(ref room) => {
                        if !is_connected {
                            self.state =
                                State::Start(StartMenu::new(addr, Some(room.username.to_string())));
                        }
                    }

                    State::Start(ref mut start_menu) => {
                        start_menu.host_input.focus(!is_connected);
                        start_menu.username_input.focus(is_connected);
                    }
                };
            }

            NetEvent::Message(message) => {
                // if let Some(game) = self.game_mut() {
                //     match *message {
                //         message::ToClient::Chat(chat) => game.chat.messages.push(chat),
                //         message::ToClient::Draw(draw) => game.canvas.draw(draw),
                //         message::ToClient::PlayerConnect(player) => game.player_list.push(player),
                //         message::ToClient::PlayerDisconnect(id) => game.player_list.retain(|player| player.name.id() != id.id()),
                //         message::ToClient::RoomStateChange(state) => game.update_state(state),
                //         message::ToClient::TurnStart(turn) => {
                //             if let Some(world) = game.state.world_mut() { world.turn = turn; }
                //         }
                //         message::ToClient::Kicked(_) => {}
                //         message::ToClient::TimeChanged(_) => {}

                //         _ => panic!("server & client state not in sync {:?}", message),
                //     };
                // } else if let message::ToClient::JoinRoom {
                //     username,
                //     player_list,
                //     initial_state,
                // } = *message
                // {
                //     self.state = State::InGameRoom(Room::new(username, player_list, initial_state));
                // }
            }
        }

        Ok(())
    }

    fn handle_input_event(&mut self, event: InputEvent) -> Result<()> {
        if event
            == InputEvent::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
            })
        {
            // close on ctrl-c
            self.exit();
        } else {
            let view = self.get_current_view();
            let action = match event {
                InputEvent::Key(key_event) => view.on_key_event(key_event),
                InputEvent::Mouse(mouse_event) => view.on_mouse_event(mouse_event),
                InputEvent::Resize(x, y) => view.on_resize((x, y)),
            };

            action(self);
        }

        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut terminal = Terminal::new(ui::backend()).unwrap();

        self.sender().send(Event::Redraw);

        while !self.should_exit {
            match self.event_queue.recv() {
                Event::Redraw => {
                    terminal.draw(|frame| self.get_current_view().draw(frame))?;
                    self.sender()
                        .send_after(Event::Redraw, self.forced_refresh_rate);
                }

                Event::Net(net_event) => self.handle_net_event(net_event).await?,

                // handle input events
                Event::Input(event) => self.handle_input_event(event)?,
            }
        }

        self.server.disconnect();

        Ok(())
    }
}
