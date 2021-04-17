use std::{
    convert::TryFrom,
    fmt::{self, Debug},
    marker::PhantomData,
};

use byteorder::ReadBytesExt;
use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

use crate::{
    world::Draw,
    world::{Game, Player, RoomState, Turn, Username},
};

/// Client -> Server
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToServer {
    Ping,
    Login(String),
    Chat(ChatMessage),
    Draw(Draw),
    RequestRoom(Option<String>, RoomRequest),
    ListRoom,
}

/// Server -> Client
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToClient {
    Chat(ChatMessage),
    Draw(Draw),
    PlayerConnect(Player),
    PlayerDisconnect(Username),
    Kicked(String),
    TurnStart(Turn),
    RoomStateChange(RoomState<Game>),
    JoinRoom {
        username: Username,
        player_list: Vec<Player>,
        initial_state: RoomState<Game>,
    },
    // GameOver(SkribblState),
    TimeChanged(u32),
    // Leave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMessage {
    System(String),
    User(Username, String),
}

impl ChatMessage {
    pub fn is_system(&self) -> bool { matches!(self, ChatMessage::System(..)) }

    pub fn username(&self) -> Option<&Username> {
        match self {
            ChatMessage::User(username, _) => Some(username),
            _ => None,
        }
    }

    pub fn inner(&self) -> &str {
        match self {
            ChatMessage::System(msg) => &msg,
            ChatMessage::User(_, msg) => &msg,
        }
    }

    pub fn into_inner(self) -> String {
        match self {
            ChatMessage::System(msg) | ChatMessage::User(_, msg) => msg,
        }
    }
}

impl fmt::Display for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatMessage::System(msg) => write!(f, "{}", msg),
            ChatMessage::User(user, msg) => write!(f, "{}: {}", user, msg),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RoomRequest {
    Find,
    Create,
    Join(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not de/serialze")]
    Serialization(#[from] bincode::Error),

    #[error("IO error")]
    IOError(#[from] std::io::Error),

    #[error("payload too large")]
    LargePayload,

    #[error("invalid frame length `0`")]
    InvalidLengthBye(u8),
}

#[derive(Debug)]
pub struct NetworkMessage<T> {
    __: PhantomData<T>,
    // codec: BytesCodec,
}

impl<T> NetworkMessage<T> {
    pub fn new() -> Self {
        Self {
            __: PhantomData,
            // codec: BytesCodec::new(),
        }
    }
}

impl<T> Decoder for NetworkMessage<T>
where
    for<'de> T: Deserialize<'de>,
{
    type Item = T;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // if let Some(data) = self.codec.decode(src)? {
        //     Ok(Some(bincode::deserialize(&data[..])?))
        // } else {
        //     Ok(None)
        // }

        if src.len() <= 3 {
            // there are no bytes to consume, stop querying the buffer
            return Ok(None);
        }

        // parse out the bytes from the start of the buffer
        let mut reader = src.as_ref();
        let header_len_size = reader.read_u8()?;

        let payload_size = match header_len_size {
            0 => {
                return Ok(None);
            }
            2 => reader.read_u16::<byteorder::BigEndian>()? as usize,
            4 => reader.read_u32::<byteorder::BigEndian>()? as usize,
            8 => reader.read_u64::<byteorder::BigEndian>()? as usize,
            _ => {
                return Err(Error::InvalidLengthBye(header_len_size));
            }
        };

        // read payload
        let header_size = 1 + header_len_size as usize;
        let current_frame_size = header_size + payload_size;

        if src.len() < current_frame_size {
            // no payload yet
            // reserve place for the current frame and the next header for better efficiency
            src.reserve(current_frame_size);
            return Ok(None);
        }

        src.advance(header_size as usize);
        let data = &src.split_to(payload_size).freeze();

        Ok(Some(bincode::deserialize(data)?))
    }
}

// +----------+----------+--------------------------------+
// | bytelen  | len: uXX |          frame payload         |
// +----------+----------+--------------------------------+
impl<T> Encoder<T> for NetworkMessage<T>
where
    T: Serialize + Debug,
{
    type Error = Error;

    fn encode(&mut self, msg: T, buf: &mut BytesMut) -> Result<(), Self::Error> {
        // self.codec
        //     .encode(bincode::serialize(&msg)?.into(), buf)
        //     .map_err(Self::Error::from)

        let msg = bincode::serialize(&msg)?;
        let msg_len = msg.len();

        // reserve space for bytelen
        buf.reserve(1);
        if u16::try_from(msg_len).is_ok() {
            buf.put_u8(2);
            buf.reserve(2);
            buf.put_u16(msg_len as u16);
        } else if u32::try_from(msg_len).is_ok() {
            buf.put_u8(4);
            buf.reserve(4);
            buf.put_u32(msg_len as u32);
        } else if u64::try_from(msg_len).is_ok() {
            buf.put_u8(8);
            buf.reserve(8);
            buf.put_u64(msg_len as u64);
        } else {
            log::error!("Net Msg payload size can't be larger than u64 can fit");
            return Err(Error::LargePayload);
        }

        buf.reserve(msg_len);
        buf.put(&msg[..]);

        Ok(())
    }
}
