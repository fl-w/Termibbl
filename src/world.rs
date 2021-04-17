use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use tui::style::Color as TuiColor;

pub type PlayerId = u8;

#[derive(
    Default, Eq, PartialEq, Hash, Clone, serde::Serialize, serde::Deserialize, Ord, PartialOrd,
)]
pub struct Username(String, PlayerId);

impl Username {
    pub fn new(name: String, id: PlayerId) -> Username { Username(name, id) }
    pub fn name(&self) -> &str { self.0.as_str() }
    pub fn id(&self) -> PlayerId { self.1 }
    pub fn into_inner(self) -> (String, PlayerId) { (self.0, self.1) }
}

impl Into<(String, PlayerId)> for Username {
    fn into(self) -> (String, PlayerId) { (self.0, self.1) }
}

impl Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Debug for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.name(), self.id())
    }
}

/// The data server stores for every player
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub score: u32,
    pub name: Username,
    pub solved_current_round: bool,
}

impl From<Username> for Player {
    fn from(name: Username) -> Self {
        Self {
            score: 0,
            name,
            solved_current_round: false,
        }
    }
}

/// A u16 point in 2D space.
pub type Coord = (u16, u16);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Draw {
    Clear,
    Erase(Coord),
    Paint { points: Vec<Coord>, color: Color },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameOpts {
    pub dimensions: Coord,
    pub number_of_rounds: usize,
    pub round_duration: usize,
    pub max_room_size: usize,
    // pub canvas_color: Color,
    pub custom_words: Vec<String>,
    pub only_custom_words: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RoomState<T> {
    FreeDraw,
    Lobby,
    Waiting,
    Playing(T),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TurnState {
    Start,
    Drawing,
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DrawingWord {
    Guess {
        /// revealed characters
        hints: HashMap<usize, char>,
        // player drawing
        who: Username,
        word_len: usize,
    },

    Draw(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub state: TurnState,
    pub word: DrawingWord,
    pub end_instant: u64,
    pub current_round: usize,
    pub last_round: usize,
}

/// Contains all info about an ongoing game
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub dimensions: Coord,
    /// a canvas is a Vec of user draw sent to the server.
    pub canvas: HashMap<Coord, Color>,
    pub turn: Turn,
}

impl From<(Username, &str)> for DrawingWord {
    fn from((who, word): (Username, &str)) -> Self {
        let mut hints = HashMap::new();

        // reveal whitespace and '-' chars
        for (idx, c) in word
            .chars()
            .enumerate()
            .filter(|(_, c)| c.is_whitespace() || c == &'-')
        {
            hints.insert(idx, c);
        }

        Self::Guess {
            word_len: word.len(),
            who,
            hints,
        }
    }
}
impl DrawingWord {}

impl Turn {
    pub fn with_word(mut self, word: DrawingWord) -> Self {
        self.word = word;

        self
    }
}

impl Game {
    pub fn remaining_round_time(&self) -> u32 {
        std::cmp::max(0, self.turn.end_instant as i64 - get_time_now() as i64) as u32
    }
}

impl<World: Clone> RoomState<World> {
    pub fn world(&self) -> Option<&World> {
        if let RoomState::Playing(ref info) = self {
            Some(info)
        } else {
            None
        }
    }

    pub fn world_mut(&mut self) -> Option<&mut World> {
        if let RoomState::Playing(ref mut info) = self {
            Some(info)
        } else {
            None
        }
    }
}

impl RoomState<Game> {
    pub fn dimensions(&self) -> Option<Coord> { self.world().map(|world| (world.dimensions)) }

    pub fn canvas_mut(&mut self) -> Option<&mut HashMap<Coord, Color>> {
        self.world_mut().map(|world| &mut world.canvas)
    }
}

pub fn get_time_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

macro_rules! derive_into {
    ($(#[$meta:meta])*
       $vis:vis enum $name: ident => $into_type: path { $($variant: ident => $into_variant: expr,)* }
     ) => {
        $(#[$meta])*
        $vis enum $name {
            $($variant),*
        }

        impl From<$name> for $into_type {
            fn from(v: $name) -> Self {
                match v {
                    $($name::$variant => $into_variant,)*
                }
            }
        }
    };
}

derive_into! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    pub enum Color => TuiColor {
        White => TuiColor::White,
        Gray => TuiColor::Gray,
        DarkGray => TuiColor::DarkGray,
        Black => TuiColor::Black,
        Red => TuiColor::Red,
        LightRed => TuiColor::LightRed,
        Green => TuiColor::Green,
        LightGreen => TuiColor::LightGreen,
        Blue => TuiColor::Blue,
        LightBlue => TuiColor::LightBlue,
        Yellow => TuiColor::Yellow,
        LightYellow => TuiColor::LightYellow,
        Cyan => TuiColor::Cyan,
        LightCyan => TuiColor::LightCyan,
        Magenta => TuiColor::Magenta,
        LightMagenta => TuiColor::LightMagenta,
    }
}
