mod cli;
mod room;
mod session;

pub use self::cli::CliOpts;
use self::room::GameRoom;

use crate::{
    events::{EventQueue, EventSender},
    message::{NetworkMessage, RoomRequest, ToClient, ToServer},
    world::{GameOpts, PlayerId, Username},
};
use futures_util::StreamExt;
use session::{InGameUser, User, UserSession};
use std::{collections::HashMap, net::SocketAddr, time::Duration};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{FramedRead, FramedWrite};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error")]
    IOError(#[from] std::io::Error),
    #[error("room not found")]
    RoomNotFound,
}

#[derive(Debug)]
pub enum Message {
    /// Notify server of game request
    RoomRequest {
        from: Username,
        req: RoomRequest,
    },
    /// Notify server of client message
    InRoomMessage {
        from: Username,
        msg: ToServer,
    },
    /// Notify server of disconnected client.
    Disconnect {
        id: PlayerId,
    },

    CtrlC,
}

pub struct GameServer {
    event_queue: EventQueue<Message>,
    /// hold game rooms by thier key
    game_rooms: HashMap<String, GameRoom>,
    // /// list of words
    // default_words: Vec<String>,
    /// holds the default game configuration
    default_game_opts: GameOpts,
    /// list of players searching for a game
    game_queue: Vec<PlayerId>,
    /// holds connected users by id
    connected_users: HashMap<PlayerId, User>,
}

impl GameServer {
    pub fn new(default_game_opts: GameOpts) -> Self {
        Self {
            event_queue: EventQueue::default(),
            game_rooms: vec![(
                "main".to_owned(),
                GameRoom::new(default_game_opts.clone(), None),
            )]
            .into_iter()
            .collect(),
            // default_words: default_game_opts.custom_words.drain(..).collect(),
            default_game_opts,
            game_queue: Vec::new(),
            connected_users: HashMap::new(),
        }
    }

    /// generate unique u8
    fn gen_unique_id(&self) -> u8 {
        // garenteed to return if max num of players is 2^8
        loop {
            let id: u8 = rand::random();

            if !self.connected_users.contains_key(&id) {
                return id;
            }
        }
    }

    /// handle stream of TcpStream's
    fn on_client_connect(&mut self, peer_addr: SocketAddr, st: TcpStream) {
        log::info!("new client connection: {}", peer_addr);

        let unique_id = self.gen_unique_id();
        let server_ref = self.event_queue.sender().clone();

        // frame socket
        let framed_socket = {
            let (r, w) = tokio::io::split(st);
            (
                FramedRead::new(r, NetworkMessage::<ToServer>::new()),
                FramedWrite::new(w, NetworkMessage::<ToClient>::new()),
            )
        };

        let session = UserSession::new(unique_id, server_ref, peer_addr, framed_socket);
        self.connected_users.insert(
            unique_id,
            User {
                sender: session.sender(),
                game: None,
                thread: tokio::spawn(async move {
                    session.run().await;
                }),
            },
        );
    }

    fn on_client_disconnect(&mut self, id: PlayerId) {
        if let Some(user) = self.connected_users.remove(&id) {
            if let Some((username, key, room)) = user.game.and_then(|game| {
                self.game_rooms
                    .get_mut(&game.room_key)
                    .map(|room| (game.name, game.room_key, room))
            }) {
                room.disconnect(id);
                log::info!("{} left the room {}", username, key);
            } else {
                log::info!("#{} left the server", id);
            }
        }
    }

    fn kick_user<S: Into<String>>(&mut self, user_id: PlayerId, reason: S) {
        if let Some(user) = self.connected_users.get_mut(&user_id) {
            if user
                .sender
                .send(session::Message(ToClient::Kicked(reason.into())))
                .is_ok()
            {
                // no need to wait for client to disconnect themselves
                self.on_client_disconnect(user_id);
            }
        }
    }

    fn on_user_game_msg(&mut self, from: Username, msg: ToServer) {
        let room_key = if let Some(key) = self
            .connected_users
            .get(&from.id())
            .and_then(|user| user.game.as_ref().map(|game| &game.room_key).cloned())
        {
            key
        } else {
            return; // potentially a naughty client - maybe kick?
        };

        if let Some(room) = self.game_rooms.get_mut(&room_key) {
            match msg {
                ToServer::Chat(chat) => return room.on_chat_msg(from, chat.into_inner()),
                ToServer::Draw(draw) => return room.on_paint_msg(from.id(), draw),
                _ => (),
            };

            self.kick_user(
                from.id(),
                "You are being naughty, got a unexpected message.",
            );
        }
    }

    fn on_room_request(&mut self, username: Username, action: RoomRequest) {
        let id = username.id();
        let user = if let Some(user) = self.connected_users.get_mut(&id) {
            user
        } else {
            // should be unreachable
            return;
        };

        if user.game.is_some() {
            // user already in game, possibly a bad client
            return self.kick_user(
                id,
                "You are not allowed to join multiple game rooms.".to_owned(),
            );
        }

        let room_key = match action {
            // TODO: allow users to create private gamerooms
            RoomRequest::Join(ref room_key) => room_key,
            _ => {
                return self.kick_user(id, "Unimplemented feature".to_owned());
            }
            // RoomRequest::Find => unimplemented!(),
            // RoomRequest::Create => unimplemented!(),
        }
        .to_owned();
        let sender = user.sender.clone();
        let name = username.name().to_owned();

        if let Err(e) = self
            .game_rooms
            .get_mut(&room_key)
            .ok_or(Error::RoomNotFound)
            .and_then(|room| room.connect(username, sender))
        {
            self.kick_user(id, format!("{:?}", e));
        } else {
            log::info!("{}", format!("{:?} joined room {}", name, room_key));

            user.game = Some(InGameUser { room_key, name });
        }
    }

    pub fn sender(&self) -> &EventSender<Message> { self.event_queue.sender() }

    /// start server listener on given address
    pub async fn listen_on(mut self, addr: &str) -> Result<()> {
        // start tcp listener :: TODO: maybe use udp or both instead?
        let mut tcp_listener = TcpListener::bind(addr)
            .await
            .expect("Could not start webserver (could not bind)")
            .map(|stream| {
                let st = stream.unwrap();
                let addr = st.peer_addr().unwrap();

                st.set_nodelay(true)
                    .expect("Failed to set stream as nonblocking");

                st.set_keepalive(Some(Duration::from_secs(1)))
                    .expect("Failed to set keepalive");

                (st, addr)
            });

        loop {
            tokio::select! {
                // listen and handle incoming connections in async thread.
                Some((socket, addr)) = tcp_listener.next() => self.on_client_connect(addr, socket),

                Some(event) = self.event_queue.recv() => {
                    match event {
                        Message::RoomRequest { from, req, } => self.on_room_request(from, req),
                        Message::InRoomMessage { from, msg } => self.on_user_game_msg(from, msg),
                        Message::Disconnect { id } => self.on_client_disconnect(id),
                    }
                }

                else => panic!("what happened???"),
            };
        }

        Ok(())
    }
}

// let state = match &mut self.game_state {
//     GameState::Skribbl(state) => state,
//     _ => return Ok(()),
// };

// let remaining_time = state.remaining_time();
// let revealed_char_cnt = state.revealed_characters().len();

// if remaining_time <= 0 {
//     let old_word = state.current_word().to_string();
//     if let Some(ref mut drawing_user) = state.player_states.get_mut(&state.drawing_user) {
//         drawing_user.score += 50;
//     }

//     state.next_turn();
//     let state = self.game_state.skribbl_state().unwrap().clone();
//     self.lines.clear();
//     tokio::try_join!(
//         self.broadcast(ToClientMsg::SkribblStateChanged(state)),
//         self.broadcast(ToClientMsg::ClearCanvas),
//         self.broadcast_system_msg(format!("The word was: \"{}\"", old_word)),
//     )?;
// } else if remaining_time <= (ROUND_DURATION / 4) as u32 && revealed_char_cnt < 2
//     || remaining_time <= (ROUND_DURATION / 2) as u32 && revealed_char_cnt < 1
// {
//     state.reveal_random_char();
//     let state = state.clone();
//     self.broadcast(ToClientMsg::SkribblStateChanged(state))
//         .await?;
// }

// self.broadcast(ToClientMsg::TimeChanged(remaining_time as u32));
