mod game;
mod lobby;

use crossterm::terminal;

use crate::{
    data::{GameInfo, Username, WordHint},
    message::{GameEvent, InitialRoomState, RoomEvent, RoomInfo},
};

use self::{game::Game, lobby::Lobby};

use super::View;

pub enum State {
    Lobby,
    Skribbl(Box<Game>),
}

pub struct Room {
    pub info: RoomInfo,
    pub state: State,
    pub lobby: Lobby,
    pub username: Username,
}

impl Room {
    pub fn new(initial_room_state: InitialRoomState) -> Self {
        let mut new = Self {
            username: initial_room_state.username,
            state: State::Lobby,
            lobby: Lobby::default(),
            info: initial_room_state.room,
        };

        if let Some(game) = initial_room_state.game {
            new.on_game_start(game);
        }

        new
    }

    fn game(&mut self) -> Option<&mut Game> {
        if let State::Skribbl(game) = &mut self.state {
            Some(game)
        } else {
            None
        }
    }

    pub fn get_current_view(&self) -> &dyn View {
        match &self.state {
            State::Lobby => &self.lobby,
            State::Skribbl(skribbl) => skribbl.as_ref(),
        }
    }

    pub fn get_current_view_mut(&mut self) -> &mut dyn View {
        match &mut self.state {
            State::Lobby => &mut self.lobby,
            State::Skribbl(skribbl) => skribbl.as_mut(),
        }
    }

    fn set_state(&mut self, state: State) {
        self.state = state;

        self.get_current_view_mut()
            .on_resize(terminal::size().unwrap());
    }

    fn on_game_start(&mut self, game: GameInfo) {
        self.set_state(State::Skribbl(Box::new(Game::new(
            game,
            &self.info.game_opts,
            self.username.id(),
        ))));
    }

    fn on_game_event(&mut self, event: GameEvent) {
        let game = if let State::Skribbl(ref mut game) = self.state {
            game
        } else {
            return;
        };

        match event {
            GameEvent::PlayerJoin(player) => {
                self.info.connected_users.push(player.name.clone());
                game.info.players.push(player);
            }
            GameEvent::WordHint((idx, ch)) => {
                if let Some(WordHint::Hint { hints, .. }) = game.info.state.as_turn_drawing_mut() {
                    hints.insert(idx, ch);
                }
            }
            GameEvent::Draw(draw) => game.canvas.apply(draw),
            GameEvent::StateUpdate(phase) => game.update_state(phase),
            GameEvent::PlayGuessed(name) => {
                game.info
                    .get_player_mut(name.id())
                    .unwrap()
                    .secs_to_solve_turn = 1
            }
            GameEvent::PlayerListUpdate(players) => {
                game.info.players = players;
            }
        }
    }

    pub fn process_event(&mut self, event: RoomEvent) {
        match event {
            RoomEvent::Chat(chat) => {
                // for now only display chat ingame
                if let Some(game) = self.game() {
                    game.chat.messages.push(chat);
                }
            }
            RoomEvent::GameEvent(game_event) => self.on_game_event(game_event),
            RoomEvent::UserJoin(username) => self.info.connected_users.push(username),
            RoomEvent::UserLeave(username) => {
                self.info.connected_users.retain(|u| u != &username);
                if let Some(game) = self.game() {
                    game.info.players.retain(|p| p.name != username);
                }
            }
            RoomEvent::StartGame(game) => self.on_game_start(game),
            RoomEvent::EndGame => self.set_state(State::Lobby),
        };
    }
}
