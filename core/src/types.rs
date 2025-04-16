use chrono::{DateTime, Utc};
use cursive::theme::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputEvent {
    TextLine(TextLine),
    ChatMessage(ChatMessage),
    SystemEvent(SystemEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl From<SerializableColor> for Color {
    fn from(color: SerializableColor) -> Self {
        Color::Rgb(color.r, color.g, color.b)
    }
}

impl From<Color> for SerializableColor {
    fn from(color: Color) -> Self {
        match color {
            Color::Rgb(r, g, b) => SerializableColor { r, g, b },
            // Handle other color variants appropriately
            _ => SerializableColor { r: 0, g: 0, b: 0 }, // Default for non-RGB colors
        }
    }
}

// Then update your TextLine struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLine {
    pub text: String,
    pub color: Option<SerializableColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub content: String,
    pub sender: String,
    pub username: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub is_self: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    ConnectionEstablished { address: String },
    ConnectionClosed,
    ConnectionError { message: String },
    PromptInput { prompt: String },
    RateLimit { seconds: f32 },
}
