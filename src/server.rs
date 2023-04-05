mod cli;
mod room;
mod session;
mod skribbl;

pub use self::cli::CliOpts;
use self::room::{GameRoom, RoomInbox, RoomMessage};

use crate::{
    data::{GameOpts, UserId, Username},
    events::{EventQueue, EventSender},
    message::RoomRequest,
    utils::{self, dispatch_abortable_task, AbortableTask},
};
use futures_util::StreamExt;
use rand::{prelude::ThreadRng, Rng};
use session::{User, UserSession};
use std::{collections::HashMap, future::Future, net::SocketAddr, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

pub const DEFAULT_WORDS: &str = include_str!("server/words.txt");

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not start webserver (could not bind)")]
    TcpBind,
}

#[derive(Debug)]
pub enum Message {
    /// Notify server of disconnected client.
    Disconnect(UserId),

    /// Notify server of room closing
    RoomClosed(String),
    LeaveQueue {
        id: UserId,
    },
}

/// store details about room
struct Room {
    inbox: RoomInbox,
    thread_handle: AbortableTask<()>,
    private: bool,
}

struct WordList {
    words: Vec<String>,
}

struct Game<'a> {
    game_opts: GameOpts,
    word_gen: fn() -> &'a str,
}

pub struct GameServer {
    tcp_listener: TcpListener,
    event_queue: EventQueue<Message>,
    /// hold the main game room
    // room: Room,
    /// holds the default game configuration
    default_game_opts: GameOpts,
    word_list: Vec<String>,
    /// holds connected users by id
    connected_users: HashMap<UserId, User>,
    /// random number generator for id & name generation
    rng: ThreadRng,
}

pub async fn run(
    port: u16,
    default_game_opts: GameOpts,
    custom_word_list: Option<String>,
) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let tcp_listener = TcpListener::bind(&addr).await.map_err(|_| Error::TcpBind)?;
    let event_queue = EventQueue::default();
    let word_list = custom_word_list
        .unwrap_or_else(|| DEFAULT_WORDS.to_string())
        .lines()
        .map(|x| x.trim().to_owned())
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();

    let server = GameServer {
        tcp_listener,
        event_queue,
        default_game_opts,
        connected_users: HashMap::new(),
        word_list,
        rng: rand::thread_rng(),
    };

    let event_tx = server.tx();

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                // return Err(err);
            } else {
                println!("🚀 Running Termibbl server on {}...", addr);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("✨ Ctrl-C received. Stopping..");
            // event_tx.send_with_urgency(Message::CtrlC);
        }
    };

    Ok(())
}

impl GameServer {
    // pub fn new(default_game_opts: GameOpts) -> Self {
    //     let default_words = cli
    //         .words
    //         .take()
    //         .unwrap_or_else(|| DEFAULT_WORDS.to_string())
    //         .lines()
    //         .map(|x| x.trim())
    //         .filter(|x| !x.is_empty())
    //         .collect::<Vec<_>>();

    //     let mut this = Self {
    //         event_queue: EventQueue::default(),
    //         room: dis,
    //         words: Arc::new(default_words),
    //         default_game_opts,
    //         game_queue: Vec::new(),
    //         connected_users: HashMap::new(),
    //     };

    //     // create default game room for NOW
    //     this.dispatch_room("default".to_owned(), None);

    //     this
    // }

    pub fn tx(&self) -> &EventSender<Message> { self.event_queue.sender() }

    /// generate unique u8
    fn gen_unique_id(&mut self) -> u8 {
        // garenteed to return if max num of players is 2^8
        loop {
            let id: u8 = self.rng.gen();
            if !self.connected_users.contains_key(&id) {
                return id;
            }
        }
    }

    fn on_client_disconnect(&mut self, id: UserId) {
        if self.connected_users.remove(&id).is_some() {
            println!("#{} left the server", id);
        }

        self.on_client_leave_queue(id);
    }

    fn kick_user<S: Into<String>>(&mut self, user_id: UserId, reason: S) {
        if let Some(user) = self.connected_users.remove(&user_id) {
            user.inbox.send(session::Message::Kick(reason.into()));
            println!("#{} kicked from the server", user_id);
        }
    }

    fn on_room_close(&mut self, key: String) {
        // if let Some(_room) = self.rooms.remove(&key) {
        //     println!("closed room {}", key)
        // }
    }

    fn dispatch_room(self, key: String, leader: Option<Username>) -> Room {
        let is_private = leader.is_some();
        let server = self.tx().clone();

        // dispatch room
        let mut room = GameRoom::new(key, self.default_game_opts.clone(), &self.words, leader);
        let room_key = room.key().to_owned();
        let sender = room.sender().clone();

        let thread_handle = utils::dispatch_abortable_task(async move {
            let room_key = room.key().to_owned();
            if let Err(e) = room.run_loop().await {
                eprintln!("room encountered error {}", e);
            }

            // notify server of room death
            server.send_with_urgency(Message::RoomClosed(room_key));
        });

        Room {
            inbox: sender,
            thread_handle,
            private: is_private,
        }
    }

    fn on_client_leave_queue(&mut self, id: UserId) {
        // if let Some((idx, name)) = self
        //     .game_queue
        //     .iter()
        //     .enumerate()
        //     .find(|(_, name)| name.id() == id)
        //     .map(|(idx, n)| (idx, n.clone()))
        // {
        //     self.game_queue.remove(idx);

        //     if let Some(user) = self.connected_users.get(&id) {
        //         user.inbox.send(session::Message::LeaveQueue);
        //     }

        //     println!("#{} left room queue...", name);
        // }
    }

    fn on_room_request(&mut self, name: Username, action: RoomRequest) {
        // let user_id = name.id();
        // let inbox = if let Some(user) = self.connected_users.get_mut(&user_id) {
        //     user.inbox.clone()
        // } else {
        //     return;
        // };

        // let room_key = match action {
        //     RoomRequest::Join(room_key) => room_key,
        //     RoomRequest::Create => {
        //         let room_key = self.gen_key();
        //         self.dispatch_room(room_key.clone(), Some(name.clone()));
        //         println!("{} created room with key {}", name, room_key);

        //         room_key
        //     }

        //     RoomRequest::Find => {
        //         println!("{} joined room queue...", name);
        //         inbox.send(session::Message::JoinQueue);
        //         self.game_queue.push(name);

        //         if self.game_queue.len() == 1 {
        //             self.tx()
        //                 .send_with_delay(Message::ClearQueue, Duration::from_secs(3));
        //         }
        //         return;
        //     }
        // };

        // if let Some(room) = self.rooms.get(&room_key) {
        //     room.inbox.send(RoomMessage::Join { name, inbox });
        // } else {
        //     inbox.send_with_urgency(session::Message::RoomNotFound);
        // }
    }

    /// handle stream of TcpStream
    fn on_client_connect(&mut self, st: TcpStream) {
        let addr = st.peer_addr().unwrap();
        let unique_id = self.gen_unique_id();
        let sender = self.tx().clone();
        let socket = utils::frame_socket(st);

        println!("new client connection: {}, id = {}", addr, unique_id);

        self.connected_users.insert(
            unique_id,
            UserSession::create_user(unique_id, addr, sender, socket),
        );
    }

    /// start server on given address
    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                Some(event) = self.event_queue.recv_async() => {
                    match event {
                        Message::LeaveQueue { id } => self.on_client_leave_queue(id),
                        Message::Disconnect(id) => self.on_client_disconnect(id),
                        Message::RoomClosed(key) => self.on_room_close(key),
                        _ => ()
                    }
                }

                // listen and accept incoming connections in async thread.
                conn = self.tcp_listener.accept() => {
                    if let Ok((socket, _)) = conn {
                        self.on_client_connect(socket)
                    } else {
                        // err occurred whilst openning socket...
                    }
                },

                // tcp pipe probably closed, stop server
                else => break,
            };
        }

        println!("server closing");

        // TODO: wait until connections are closed before returing...

        // disconnect users
        for (_, user) in self.connected_users.drain() {
            user.inbox
                .send_with_urgency(session::Message::Kick("Server Shutdown".into()));
        }

        // // close of game rooms
        // for (_, room) in self.rooms.drain() {
        //     room.inbox.send_with_urgency(RoomMessage::Close);
        //     room.thread_handle.abort(); // dont wait for room to finish
        // }

        Ok(())
    }
}
