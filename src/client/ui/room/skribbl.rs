use crate::{
    client::ui::{
        canvas::{Palette, TermCanvas, PALETTE},
        input::InputText,
    },
    message::ChatMessage,
    world::Game,
};

#[derive(Debug, Clone, Default)]
pub struct Chat {
    pub input: InputText,
    pub messages: Vec<ChatMessage>,
}

impl Chat {
    pub fn new() -> Self {
        let mut new = Self {
            ..Default::default()
        };
        new.input.focus(true);
        new
    }
}

pub struct Skribbl {
    pub game: Game,
    pub chat: Chat,
    pub canvas: TermCanvas,
    pub palette: Palette,
}

impl Skribbl {
    fn new(game: Game) -> Self {
        Self {
            chat: Chat::new(),
            canvas: TermCanvas::default(),
            palette: Palette::new(PALETTE),
            game,
        }
    }
}
