use crate::{
    events::{EventQueue, EventSender},
    message::{NetworkMessage, ToClient, ToServer},
    server::Message as ServerMessage,
    world::{PlayerId, Username},
};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::{
    io::{ReadHalf, WriteHalf},
    net::TcpStream,
    task::JoinHandle,
};
use tokio_util::codec::{FramedRead, FramedWrite};

/// Disconnect client after this many seconds of no heartbeat
pub const TIMED_OUT_SECONDS: u64 = 10;

type ClientMessageWriter = FramedWrite<WriteHalf<TcpStream>, NetworkMessage<ToClient>>;
type ClientMessageReader = FramedRead<ReadHalf<TcpStream>, NetworkMessage<ToServer>>;

pub type Sender = EventSender<Message>;

/// Chat server sends this messages to session
#[derive(Debug)]
pub struct Message(pub ToClient);

#[derive(Clone)]
pub enum UserState {
    Idle,
    // InQueue {
    //     name: Username,
    // },
    InGame { name: String },
    Stop,
}

/// `UserSession` actor is responsible for TCP peer communications.
pub struct UserSession {
    /// unique session id
    id: PlayerId,
    /// socket address
    peer_addr: SocketAddr,
    /// client state
    state: UserState,
    /// this is the event queue for this session
    event_queue: EventQueue<Message>,
    /// this is sender for server event queue
    server: EventSender<ServerMessage>,
    /// Framed sockets
    framed: (ClientMessageReader, ClientMessageWriter),
    /// client must send a message at least once every 5 seconds
    last_hb: Instant,
}

pub struct InGameUser {
    pub room_key: String,
    pub name: String,
}

pub struct User {
    pub sender: Sender,
    pub game: Option<InGameUser>,
    pub thread: JoinHandle<()>,
}

impl UserSession {
    const NAMES: [&'static str; 4] = ["alice", "bob", "dafny", "spice"];

    pub fn new(
        id: PlayerId,
        server: EventSender<super::Message>,
        peer_addr: SocketAddr,
        client_msg_stream: (ClientMessageReader, ClientMessageWriter),
    ) -> Self {
        Self {
            id,
            server,
            peer_addr,
            framed: client_msg_stream,
            event_queue: EventQueue::default(),
            state: UserState::Idle,
            last_hb: Instant::now(),
        }
    }

    fn generate_name() -> String {
        let mut rng = rand::thread_rng();
        Self::NAMES[rng.gen_range(0, Self::NAMES.len())].to_owned()
    }

    fn sender(&self) -> &Sender { self.event_queue.sender() }

    fn writer(&mut self) -> &mut ClientMessageWriter { &mut self.framed.1 }

    /// Forward server message to this client
    async fn send(&mut self, msg: ToClient) {
        log::trace!("({}): writing message <> {:?}", self.peer_addr, msg);
        match &msg {
            ToClient::JoinRoom { ref username, .. } => {
                self.state = UserState::InGame {
                    name: username.name().to_owned(),
                };
            }

            ToClient::Kicked(ref reason) => {
                log::debug!("({}): received kick signal <> {}", self.peer_addr, reason);
                self.state = UserState::Stop;
            }

            _ => {}
        };

        if let Err(err) = self.writer().send(msg).await {
            log::error!("{:?}", err);
        }
    }

    /// Handle messages from the tcp stream of the client (Client -> Server)
    async fn handle_msg(&mut self, msg: ToServer) {
        log::trace!("({}): processing message <> {:?}", self.peer_addr, msg);

        if let ToServer::Ping = msg {
            self.last_hb = Instant::now();
            return;
        }

        match &self.state {
            UserState::Idle => match msg {
                ToServer::RequestRoom(maybe_name, req) => {
                    let from =
                        Username::new(maybe_name.unwrap_or_else(Self::generate_name), self.id);

                    self.server
                        .send(ServerMessage::RoomRequest { from, req })
                        .await
                        .unwrap();
                }
                ToServer::ListRoom => {}
                _ => (), // TODO: recieved weird messaage from client, is client laggin? maybe disconnect
            },

            // UserState::InQueue { username } => {}
            UserState::InGame { name } => {
                let from = Username::new(name.clone(), self.id);

                self.server
                    .send(ServerMessage::InRoomMessage { from, msg })
                    .await
                    .unwrap();
            }

            _ => (),
        }
    }

    pub async fn run(mut self) {
        log::debug!("started thread for client {}", self.peer_addr);
        let mut hb = tokio::time::interval(Duration::from_secs(TIMED_OUT_SECONDS));

        while !matches!(self.state, UserState::Stop) {
            let client_msg = self.framed.0.next();
            let server_msg = self.event_queue.recv();

            // check client heartbeats
            if Instant::now().duration_since(self.last_hb) > Duration::new(TIMED_OUT_SECONDS, 0) {
                // heartbeat timed out
                log::info!(
                    "({}): Client heartbeat failed, disconnecting!",
                    self.peer_addr
                );

                break;
            }

            tokio::select! {
                // force loop to contiue
                _ = hb.tick() => (),

                Some(msg) = client_msg => {
                     match msg {
                         Ok(msg) => self.handle_msg(msg).await,
                         Err(err) => {
                            log::error!("decode err {:?}", err);
                            break;
                         }
                     }
                }

                // Handler for Message, server sends this message, we just send forward to
                // peer
                Some(msg) = server_msg => self.send(msg.0).await,

                else => break,
            }
        }

        // notify server
        self.server
            .send(ServerMessage::Disconnect { id: self.id })
            .await
            .unwrap();

        log::debug!("stopped thread for {}.", self.peer_addr,);
    }
}
