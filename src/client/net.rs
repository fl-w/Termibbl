use std::{
    net::SocketAddr,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use futures_util::TryFutureExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    task::JoinHandle,
};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};

use crate::{
    events::{EventQueue, EventSender},
    message::{self, NetworkMessage},
};

use super::{
    error::{Error, Result},
    Event,
};

#[derive(Debug, Copy, Clone)]
pub enum ConnectionStatus {
    NotConnected,
    Connecting,
    Connected,
    NotFound,
    Dropped,
    TimedOut,
}

impl Default for ConnectionStatus {
    fn default() -> Self { Self::NotConnected }
}

#[derive(Debug)]
pub enum NetEvent {
    Connected(ServerSession),
    Status(ConnectionStatus),
    Message(Box<message::ToClient>),
}

#[derive(Debug)]
pub struct ServerSession {
    server_addr: SocketAddr,
    server_msg_sender: EventSender<message::ToServer>,
    should_stop: Arc<AtomicBool>,
    join_handle: JoinHandle<()>,
}

impl ServerSession {
    pub fn send_server_msg(&mut self, message: message::ToServer) {
        self.server_msg_sender.send(message)
    }
}

impl Drop for ServerSession {
    fn drop(&mut self) {
        self.should_stop
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct AppServer {
    session: Option<ServerSession>,
    connection_status: ConnectionStatus,
    connection_attempt_handle: Option<JoinHandle<()>>,
}

impl AppServer {
    pub fn is_connected(&self) -> bool {
        self.session.is_some() && matches!(self.connection_status, ConnectionStatus::Connected)
    }

    pub fn connection_status(&self) -> ConnectionStatus {
        if self.connection_attempt_handle.is_some() {
            ConnectionStatus::Connecting
        } else {
            self.connection_status
        }
    }

    pub fn send_message(&mut self, message: message::ToServer) {
        if let Some(ref mut session) = self.session {
            // TODO: check if disconnected
            session.send_server_msg(message);
        }
    }

    pub fn addr(&self) -> Option<String> {
        self.session.as_ref().map(|s| s.server_addr.to_string())
    }

    pub fn set_status(&mut self, status: ConnectionStatus) {
        if !matches!(status, ConnectionStatus::Connected) {
            self.disconnect()
        }

        self.connection_status = status;
    }

    pub(crate) async fn set_session(&mut self, session: ServerSession) -> Result<()> {
        self.connection_status = ConnectionStatus::Connected;
        self.session = Some(session);

        if let Some(handle) = self.connection_attempt_handle.take() {
            handle.await?;
        }

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connection_status = ConnectionStatus::NotConnected;
        self.connection_attempt_handle.take();
        self.session.take();
    }

    /// attempt to connect to termibbl server
    pub fn connect(&mut self, server_addr: SocketAddr, mut app_tx: EventSender<Event>) {
        let mut app_tx_clone = app_tx.clone();

        if self.is_connected() {
            self.disconnect();
        }

        self.connection_attempt_handle.replace(tokio::spawn(
            TcpStream::connect(server_addr.clone())
                .map_ok(|socket| {
                    socket.set_nodelay(true).unwrap();
                    let (r, w) = socket.into_split();

                    (
                        FramedRead::new(r, NetworkMessage::<message::ToClient>::new()),
                        FramedWrite::new(w, NetworkMessage::<message::ToServer>::new()),
                    )
                })
                .map_err(Error::from)
                // TODO: verify this is a Termibbl server and versions are compatible
                .and_then(
                    |(server_to_client_reader, client_to_server_writer)| async move {
                        let session = ServerSession::create(
                            server_addr,
                            app_tx.clone(),
                            server_to_client_reader
                                .map_ok(|v| Event::Net(NetEvent::Message(Box::new(v)))),
                            client_to_server_writer,
                        );

                        app_tx
                            .send(NetEvent::Connected(session))
                            .await
                            .map_err(Error::from)
                    },
                )
                .map(move |result: Result<()>| {
                    if let Err(err) = result {
                        let status = match err {
                            Error::SendError(_) => ConnectionStatus::NotConnected,
                            Error::IOError(err) => match err.kind() {
                                std::io::ErrorKind::TimedOut => ConnectionStatus::TimedOut,
                                _ => ConnectionStatus::NotFound,
                            },
                            _ => unreachable!(),
                        };

                        app_tx_clone.send(Event::Net(NetEvent::Status(status)));
                    }
                }),
        ));
    }
}

impl ServerSession {
    fn create<R: AsyncRead, W: AsyncWrite, D: Decoder, E: Encoder<message::ToServer>>(
        server_addr: SocketAddr,
        mut app_tx: EventSender<Event>,
        server_to_client: FramedRead<R, D>,
        mut client_to_server: FramedWrite<W, E>,
    ) -> Self {
        let should_stop_task = Arc::new(AtomicBool::new(false));
        let mut server_to_client = server_to_client;
        let mut event_queue = EventQueue::<message::ToServer>::default();

        let server_msg_tx = event_queue.sender().clone();

        let join_handle = tokio::spawn(Self::handle(should_stop_task.clone()).and_then(
            |status| async {
                should_stop_task.store(false, std::sync::atomic::Ordering::Relaxed);
                app_tx.send(NetEvent::Status(status)).await.unwrap();
            },
        ));

        Self {
            join_handle,
            server_msg_sender: server_msg_tx,
            should_stop: should_stop_task,
            server_addr,
        }
    }

    async fn handle(should_stop_task: Arc<AtomicBool>) {
        let mut heartbeat = tokio::time::interval(Duration::from_secs(4));

        let connection_status = loop {
            if should_stop_task.load(std::sync::atomic::Ordering::Relaxed) {
                break ConnectionStatus::NotConnected;
            }

            tokio::select! {
            // send heartbeats every second otherwise server will disconnect
            _ = heartbeat.tick() => event_queue.sender().try_send(message::ToServer::Ping).unwrap(),

            // Some(server_msg) = server_to_client.next() => {
            //     if server_msg.is_err() || app_tx.try_send(server_msg.unwrap()).is_err() {
            //         break ConnectionStatus::Dropped;
            //     }
            // }

            // Some(to_server_msg) = event_queue.recv() => {
            //     if let Err(err) = client_to_server.send(to_server_msg).await {
            //         println!("client->server err: {:?}", err);
            //         break ConnectionStatus::Dropped;
            //     }
            // }

            else => break ConnectionStatus::NotConnected,
            };
        };
    }
}
