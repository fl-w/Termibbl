use std::{net::SocketAddr, time::Duration};

<<<<<<< Updated upstream
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, EventStream, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use tui::Terminal;

use crate::{
    data::Username,
    events::{EventQueue, EventSender},
    message::ToClient,
    utils::{self, AbortableTask},
};

use super::{
    app_server::{AppServer, NetEvent},
    error::{Error, Result},
    ui::{self, Room, StartMenu, View},
    InputEvent,
};

pub enum Event {
    Tick,
    Input(InputEvent),
    Net(NetEvent),
    CtrlC,
    CloseNotification,
}

enum State {
    Start(StartMenu),
    InGameRoom(Box<Room>),
}

impl Default for State {
    fn default() -> Self { Self::Start(StartMenu::new()) }
}

impl From<Room> for State {
    fn from(v: Room) -> Self { Self::InGameRoom(Box::new(v)) }
}

#[derive(Default)]
=======
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

>>>>>>> Stashed changes
pub struct App {
    event_queue: EventQueue<Event>,
    state: State,
    server: AppServer,
    should_exit: bool,
<<<<<<< Updated upstream
    notifications: Vec<String>,
}

impl App {
    pub fn sender(&self) -> &EventSender<Event> { self.event_queue.sender() }

    pub fn server(&self) -> &AppServer { &self.server }

    pub fn exit(&mut self) { self.should_exit = true; }

    pub fn set_name_input(&mut self, name: String) {
        if let State::Start(start_menu) = &mut self.state {
            start_menu.username_input.set_content(name);
        }
    }

    pub fn set_host_input(&mut self, addr: String) {
        if let State::Start(start_menu) = &mut self.state {
            start_menu.host_input.set_content(addr);
        }
    }

    pub fn get_current_view(&self) -> &dyn View {
        match &self.state {
            State::Start(start_menu) => start_menu,
            State::InGameRoom(room) => room.get_current_view(),
        }
    }

    fn get_current_view_mut(&mut self) -> &mut dyn View {
        match &mut self.state {
            State::Start(start_menu) => start_menu,
            State::InGameRoom(room) => room.get_current_view_mut(),
        }
    }

    pub fn connect_to_server(&mut self, addr: SocketAddr) {
        self.server.connect(addr, self.event_queue.sender().clone());
    }

    pub fn username(&self) -> Option<&Username> {
        match &self.state {
            State::InGameRoom(room) => Some(&room.username),
            _ => None,
        }
    }

    fn go_back(&mut self) {
        match &mut self.state {
            State::InGameRoom(_) => {
                self.server
                    .send_message(crate::message::ToServer::LeaveRoom);
                self.goto_start();
            }

            State::Start(menu) => {
                if menu.host_input.has_focus() {
                    self.exit();
                } else if menu.username_input.has_focus() {
                    self.server.disconnect();
                } else if menu.in_queue() {
                    self.server
                        .send_message(crate::message::ToServer::LeaveQueue);
                }
            }
        };
    }

    fn goto_start(&mut self) {
        let username = if let State::InGameRoom(room) = &mut self.state {
            Some(room.username.to_string())
        } else {
            None
        };

        let mut start = StartMenu::new();

        if let Some(host) = self.server.addr() {
            start.host_input.set_content(host);
            start.host_input.focus(false);
            start.username_input.focus(true);
        }

        if let Some(username) = username {
            start.username_input.set_content(username);
        }

        self.state = State::Start(start);
    }

    fn display_notification(&mut self, error: String) {
        self.notifications.push(error);
        self.sender()
            .send_with_delay(Event::CloseNotification, Duration::from_secs(4));
    }

    fn handle_net_event(&mut self, event: NetEvent) -> Result<()> {
        match event {
            NetEvent::SessionStart(session) => {
                self.server.set_session(session)?;

                let is_connected = self.server.is_connected();
                if let State::Start(state) = &mut self.state {
                    state.on_connection_status_changed(is_connected);
                }
            }

            NetEvent::Status(status) => {
                self.server.set_status(status);

                let is_connected = self.server.is_connected();
                match &mut self.state {
                    State::InGameRoom(ref room) => {
                        if !is_connected {
                            self.goto_start();
                        }
                    }

                    State::Start(ref mut start_menu) => {
                        start_menu.on_connection_status_changed(is_connected);
                    }
                };
            }

            NetEvent::Message(message) => {
                if let ToClient::Disconnect(reason) = *message {
                    // if server disconnects this client, display notification
                    // detailing reason for disconnection.
                    self.display_notification(reason);
                } else {
                    match &mut self.state {
                        State::Start(start_menu) => {
                            match *message {
                                ToClient::JoinRoom(initial_room_state) => {
                                    self.state = Room::new(initial_room_state).into();
                                }
                                ToClient::JoinQueue => start_menu.join_queue(),
                                ToClient::LeaveQueue => start_menu.leave_queue(),
                                _ => {
                                    // server sent unknown message ...
                                    return Err(Error::UnimplementedFeature(format!(
                                        "unimplemented event message {:?}",
                                        *message
                                    )));
                                }
                            };
                        }
                        State::InGameRoom(room) => {
                            match *message {
                                ToClient::RoomEvent(event) => room.process_event(event),
                                ToClient::LeaveRoom(maybe_reason) => {
                                    // kick to start screen
                                    self.goto_start();
                                    if let Some(reason) = maybe_reason {
                                        self.display_notification(reason)
                                    }
                                }

                                _ => {
                                    return Err(Error::UnimplementedFeature(format!(
                                        "unimplemented in room event message {:?}",
                                        *message
                                    )));
                                }
                            };
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_input_event(&mut self, event: InputEvent) -> Result<()> {
        if event == InputEvent::Key(KeyCode::Esc.into()) {
            self.go_back();
        } else {
            let view = self.get_current_view_mut();
            let action = match event {
                InputEvent::Key(key_event) => view.on_key_event(key_event),
                InputEvent::Mouse(mouse_event) => view.on_mouse_event(mouse_event),
                InputEvent::Resize(x, y) => {
                    view.on_resize((x, y));
                    return Ok(());
                }
            };

            action(self);
        }

        Ok(())
    }

    fn setup_input_events(&self) -> AbortableTask<()> {
        let sender = self.sender().clone();
        let ctrl_c = InputEvent::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
        });
        let mut reader = EventStream::new();

        utils::dispatch_abortable_task(async move {
            loop {
                let event = reader.next();

                if let Some(Ok(mut event)) = event.await {
                    // handle ctrl_c
                    if event == ctrl_c {
                        sender.send_with_urgency(Event::CtrlC);
                    } else if let InputEvent::Resize(x, y) = &mut event {
                        // Resize events can occur in batches.
                        // With a simple loop they can be flushed.
                        // return whether resize event was flushed
                        while let Ok(true) = crossterm::event::poll(Duration::from_millis(50)) {
                            if let Ok(InputEvent::Resize(resize_x, resize_y)) =
                                crossterm::event::read()
                            {
                                *x = resize_x;
                                *y = resize_y;
                            }
                        }
                        sender.send_with_urgency(Event::Input(event))
                    } else {
                        sender.send(Event::Input(event))
                    }
                } else {
                    break;
                }
            }
        })
    }

    /// Start the main loop.
    ///
    /// This will listen to events and do the appropriate actions.
    async fn start_loop(&mut self) -> Result<()> {
        let tick_interval = Duration::from_secs(1);
        let mut terminal = Terminal::new(ui::backend()).unwrap();

        self.sender().send(Event::Tick);
        while !self.should_exit {
            terminal.draw(|frame| self.get_current_view().draw(frame, self))?;

            match self.event_queue.recv_async().await.unwrap() {
                // handle tick
                Event::Tick => {
                    if let State::Start(start_menu) = &mut self.state {
                        start_menu.tick()
                    };

                    self.sender().send_with_delay(Event::Tick, tick_interval);
                }

                // handle network events
                Event::Net(net_event) => self.handle_net_event(net_event)?,

                // handle input events
                Event::Input(event) => self.handle_input_event(event)?,

                // close notification
                Event::CloseNotification => {
                    self.notifications.pop();
                }

                // close on ctrl-c
                Event::CtrlC => self.exit(),
            }
        }

=======
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

>>>>>>> Stashed changes
        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut stdout = std::io::stdout();

        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let input_task_handle = self.setup_input_events();
        // TODO: display notifications if any.

        let result = self.start_loop().await;
        self.server.disconnect();

        // stop listening for inputs
        input_task_handle.abort();

        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        disable_raw_mode()?;

        result
    }
}
