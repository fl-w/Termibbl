use core::cmp::min;
use std::collections::HashMap;

use rand::prelude::{IteratorRandom, SliceRandom};

use crate::{
    message::{ChatMessage, ToClient},
    world::Draw,
    world::{DrawingWord, Game, Player, PlayerId, RoomState, Turn, TurnState, Username},
};

use super::{
    cli::ROUND_DURATION,
    session::{Message, Sender},
    GameOpts, Result,
};

const REQUIRED_PLAYERS: usize = 1;

pub struct PlayerSession {
    addr: Sender,
    player: Player,
}

impl PlayerSession {
    fn send_message(&mut self, msg: Message) {
        if let Err(e) = self.addr.send(msg) {
            // maybe player has been disconnected
            log::error!("{:?}", e)
        }
    }
}

pub struct GameRoom {
    /// state of this room
    state: RoomState<Skribbl>,

    /// options of this room
    game_opts: GameOpts,

    /// the leader of this room
    owner_id: Option<PlayerId>,

    /// player sessions connected to this room
    connected_sessions: HashMap<PlayerId, PlayerSession>,
}

/// helpful functions for `GameServer`
impl GameRoom {
    pub fn new(game_opts: GameOpts, owner_id: Option<PlayerId>) -> Self {
        Self {
            state: if owner_id.is_some() {
                RoomState::Lobby
            } else {
                RoomState::Waiting
            },
            game_opts,
            owner_id,
            connected_sessions: HashMap::new(),
        }
    }

    /// send a `ToClient` to a specific session
    fn send(&mut self, player_id: PlayerId, msg: ToClient) {
        if let Some(pl) = self.connected_sessions.get_mut(&player_id) {
            pl.send_message(Message(msg));
        }
    }

    /// send a `ChatMessage::System` to a specific session
    fn send_system_msg<T: Into<String>>(&mut self, player_id: PlayerId, msg: T) {
        if let Some(pl) = self.connected_sessions.get_mut(&player_id) {
            pl.send_message(Message(ToClient::Chat(ChatMessage::System(msg.into()))));
        }
    }

    /// broadcast a `ToClient` to all connected players
    fn broadcast(&mut self, msg: ToClient) {
        for (_, session) in self.connected_sessions.iter_mut() {
            session.send_message(Message(msg.clone()));
        }
    }

    /// broadcast a `ToClient` to all connected players
    fn broadcast_except(&mut self, msg: ToClient, player_id: PlayerId) {
        for (_, session) in self
            .connected_sessions
            .iter_mut()
            .filter(|(id, _)| *id != &player_id)
        {
            session.send_message(Message(msg.clone()));
        }
    }

    /// send a ChatMessage::SystemMsg to all active sessions in room
    fn broadcast_msg(&mut self, msg: ChatMessage) { self.broadcast(ToClient::Chat(msg)) }

    /// send a ChatMessage::SystemMsg to all active sessions in room
    pub fn broadcast_system_msg(&mut self, msg: String) {
        self.broadcast(ToClient::Chat(ChatMessage::System(msg)))
    }

    fn player_list(&self) -> Vec<Player> {
        self.connected_sessions
            .values()
            .map(|session| session.player.clone())
            .collect()
    }

    fn skribbl(&self) -> Option<&Skribbl> {
        if let RoomState::Playing(ref skribbl) = self.state {
            Some(skribbl)
        } else {
            None
        }
    }

    fn skribbl_mut(&mut self) -> Option<&mut Skribbl> {
        if let RoomState::Playing(ref mut skribbl) = self.state {
            Some(skribbl)
        } else {
            None
        }
    }

    fn get_non_guessing_players(&self) -> Vec<&PlayerSession> {
        self.connected_sessions
            .values()
            .filter(|u| {
                u.player.solved_current_round
                    || self
                        .skribbl()
                        .map(|skribbl| skribbl.is_drawing(u.player.name.id()))
                        .unwrap_or_default()
            })
            .collect()
    }

    fn game_state(&self) -> RoomState<Game> {
        match &self.state {
            RoomState::FreeDraw => RoomState::FreeDraw,
            RoomState::Lobby => RoomState::Lobby,
            RoomState::Waiting => RoomState::Waiting,
            RoomState::Playing(ref skribbl) => RoomState::Playing(skribbl.game.clone()),
        }
    }

    fn update_state(&mut self, state: RoomState<Skribbl>) {
        self.state = state;
        self.broadcast(ToClient::RoomStateChange(self.game_state()));
    }

    pub fn disconnect(&mut self, player_id: PlayerId) {
        if let Some(session) = self.connected_sessions.remove(&player_id) {
            let username = session.player.name;

            // maybe let the client handle the message?
            self.broadcast_system_msg(format!("{} left the rooom", username));
            self.broadcast(ToClient::PlayerDisconnect(username));

            if self.player_list().is_empty() {
                self.end_game();
            } else if let RoomState::Playing(ref skribbl) = self.state {
                if skribbl.is_drawing(player_id) {
                    return self.start_turn();
                }
            }
        }
    }

    pub fn connect(&mut self, username: Username, addr: Sender) -> Result<()> {
        let player = Player {
            name: username.clone(),
            score: 0,
            solved_current_round: false,
        };

        self.broadcast(ToClient::PlayerConnect(player.clone()));
        self.connected_sessions
            .insert(username.id(), PlayerSession { addr, player });

        let join_msg = format!("{} joined", username);
        let player_list = self.player_list();
        let initial_state = self.game_state();

        // send joining player initial game state
        self.send(
            username.id(),
            ToClient::JoinRoom {
                username,
                player_list,
                initial_state,
            },
        );

        // TODO: is this neccesary, could be done on client.
        self.broadcast_system_msg(join_msg);

        if matches!(self.state, RoomState::Waiting) {
            self.start_game();
        }

        Ok(())
    }

    pub fn start_game(&mut self) {
        if self.connected_sessions.len() >= REQUIRED_PLAYERS {
            self.update_state(RoomState::Playing(Skribbl::new(&self.game_opts)));
            self.start_round();
        }
    }

    pub fn start_round(&mut self) {
        let player_list = self.player_list();
        let skribbl = self.skribbl_mut().expect("start turn with no game");

        if skribbl.is_last_round() {
            self.end_game()
        } else {
            skribbl.start_round(&player_list);
            let turn = skribbl.game.turn.clone();
            let drawing_user = skribbl.get_drawing_player();
            let current_word = skribbl.current_word.clone();

            self.broadcast_except(ToClient::TurnStart(turn.clone()), drawing_user);
            self.send(
                drawing_user,
                ToClient::TurnStart(turn.with_word(DrawingWord::Draw(current_word))),
            )
        }
    }

    pub fn start_turn(&mut self) {
        let skribbl = self.skribbl_mut().expect("start turn with no game");

        if skribbl.is_last_turn() {
            self.start_round();
        } else {
            skribbl.next_turn();
        }
    }

    pub fn end_game(&mut self) {
        log::debug!("game room ending.");

        self.update_state(if self.owner_id.is_some() {
            RoomState::Lobby
        } else {
            RoomState::Waiting
        });
    }

    pub fn on_paint_msg(&mut self, sender_id: PlayerId, draw_action: Draw) {
        if let RoomState::Playing(ref mut skribbl) = self.state {
            // only process draw message is player can draw
            if !skribbl.is_drawing(sender_id) {
                return;
            }

            let canvas = &mut skribbl.game.canvas;

            // update server game state
            match &draw_action {
                Draw::Clear => canvas.clear(),
                Draw::Paint { points, color } => {
                    for point in points {
                        canvas.insert(*point, *color);
                    }
                }
                Draw::Erase(point) => {
                    canvas.remove(point);
                }
            };

            self.broadcast_except(ToClient::Draw(draw_action), sender_id);
        }
    }

    pub fn on_chat_msg(&mut self, sender: Username, chat_msg: String) {
        if let RoomState::Playing(ref mut skribbl) = self.state {
            let session = self.connected_sessions.get_mut(&sender.id()).unwrap();

            // whether the given player can guess in the current turn.
            let player_can_guess =
                !(skribbl.is_drawing(sender.id()) || session.player.solved_current_round);

            if player_can_guess {
                let player = &mut session.player;

                match skribbl.do_guess(player, &chat_msg) {
                    // TODO: on correct guess, let users know that score has gone up?
                    0 => {
                        // if !self.has_any_solved() {
                        //     // half time left on solve
                        //     self.game_state.turn_end_time -= remaining_time as u64 / 2;
                        // }
                        self.broadcast_system_msg(format!("{} guessed it!", sender));
                    }

                    1 => self.send_system_msg(sender.id(), "You're very close!".to_string()),
                    _ => self.broadcast_msg(ChatMessage::User(sender, chat_msg)),
                };
            } else {
                // player cannot guess, send message to all users who can't
                for player in self.get_non_guessing_players() {
                    player
                        .addr
                        .clone()
                        .send(Message(ToClient::Chat(ChatMessage::User(
                            sender.clone(),
                            chat_msg.clone(),
                        ))))
                        .unwrap();
                }
            }
        } else {
            // let everyone know message has been sent
            self.broadcast_msg(ChatMessage::User(sender, chat_msg));
        }
    }

    pub async fn run(self) { loop {} }
}

pub struct Skribbl {
    /// the current game state
    game: Game,

    /// current word to guess
    current_word: String,

    /// players which didn't draw yet in the current round.
    pub players_left_in_round: Vec<Username>,

    // pub round_end_time: u64,
    pub words: Box<dyn Iterator<Item = String>>,
}

impl Skribbl {
    pub fn new(opts: &GameOpts) -> Self {
        let mut words = opts.custom_words.clone();

        words.shuffle(&mut rand::thread_rng());
        let turn = Turn {
            last_round: opts.number_of_rounds,
            state: TurnState::Drawing,
            word: DrawingWord::Draw(String::new()),
            end_instant: 0,
            current_round: 0,
        };

        Skribbl {
            game: Game {
                dimensions: opts.dimensions,
                turn,
                canvas: Default::default(),
            },
            current_word: String::new(),
            players_left_in_round: Vec::new(),
            words: Box::new(opts.custom_words.clone().into_iter().cycle()),
        }
    }

    fn is_last_turn(&self) -> bool { self.players_left_in_round.len() <= 1 }

    fn is_last_round(&self) -> bool {
        let Turn {
            current_round,
            last_round,
            ..
        } = &self.game.turn;

        current_round == last_round
    }

    fn start_round(&mut self, players: &[Player]) {
        let Turn { current_round, .. } = &mut self.game.turn;

        *current_round += 1;

        self.players_left_in_round = players.iter().map(|p| p.name.clone()).collect();
        self.next_turn();
    }

    fn next_turn(&mut self) {
        // self.game_info.round_end_time = self::get_time_now() + ROUND_DURATION;
        let words = &mut self.words;
        // let word = words.choose(&mut rand::thread_rng()).unwrap();

        self.current_word = words.next().unwrap();
        self.game.turn.word = (
            self.players_left_in_round.remove(0),
            self.current_word.as_str(),
        )
            .into();
    }

    fn get_drawing_player(&self) -> PlayerId {
        if let DrawingWord::Guess { who, .. } = &self.game.turn.word {
            who.id()
        } else {
            unreachable!()
        }
    }

    fn end_turn(&mut self, players: &mut Vec<Player>) {
        let remaining_time = self.game.remaining_round_time();

        for player in players {
            // TODO: score algo.. needs work
            player.score += 50;
            player.score +=
                calculate_score_increase(remaining_time, self.is_drawing(player.name.id()));
        }

        // if self.remaining_users.len() == 0 {
        //     self.remaining_users = self.player_states.keys().cloned().collect();
        // }
    }

    fn end_game(&mut self) {}

    /// reveals a random character, as long as that doesn't reveal half of the word
    pub fn reveal_random_char(&mut self) {
        if let DrawingWord::Guess {
            hints,
            who: _,
            word_len: ref word_length,
        } = &mut self.game.turn.word
        {
            if !hints.len() < word_length / 2 {
                // cant reveal char
                return;
            }

            let mut rng = rand::thread_rng();

            let (idx, ch) = self
                .current_word
                .chars()
                .enumerate()
                .filter(|(idx, _)| !hints.contains_key(&idx))
                .choose(&mut rng)
                .unwrap();

            hints.insert(idx, ch);
        } else {
            unreachable!();
        };
    }

    fn is_drawing(&self, id: PlayerId) -> bool { self.get_drawing_player() == id }

    /// try guess for a player by username, returns distance of guess
    fn do_guess(&mut self, player: &mut Player, guess: &str) -> usize {
        let remaining_time = self.game.remaining_round_time();
        let dist = levenshtein_distance(guess, &self.current_word);

        if dist == 0 {
            player.score +=
                50 + calculate_score_increase(remaining_time, self.is_drawing(player.name.id()));
        }

        dist
    }
}

fn calculate_score_increase(remaining_time: u32, _is_drawing: bool) -> u32 {
    50 + (((remaining_time as f64 / ROUND_DURATION as f64) * 100f64) as u32 / 2u32)
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let w1 = a.chars().collect::<Vec<_>>();
    let w2 = b.chars().collect::<Vec<_>>();

    let a_len = w1.len() + 1;
    let b_len = w2.len() + 1;

    let mut matrix = vec![vec![0]];

    for i in 1..a_len {
        matrix[0].push(i);
    }
    for j in 1..b_len {
        matrix.push(vec![j]);
    }

    for (j, i) in (1..b_len).flat_map(|j| (1..a_len).map(move |i| (j, i))) {
        let x: usize = if w1[i - 1].eq_ignore_ascii_case(&w2[j - 1]) {
            matrix[j - 1][i - 1]
        } else {
            1 + min(
                min(matrix[j][i - 1], matrix[j - 1][i]),
                matrix[j - 1][i - 1],
            )
        };
        matrix[j].push(x);
    }
    matrix[b_len - 1][a_len - 1]
}
