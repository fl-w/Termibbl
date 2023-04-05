use crate::message::ChatMessage;

use super::input::InputField;

#[derive(Debug, Clone, Default)]
pub struct Chat {
    pub input: InputField,
    pub messages: Vec<ChatMessage>,
}

impl Chat {
    pub fn new() -> Self {
        let mut new = Self::default();

        new.input.focus(true);
        new
    }
}
