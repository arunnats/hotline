use crate::shared::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub content: String,
    pub sender: String,
    pub username: Option<String>,
    pub timestamp: DateTime<Utc>,
}
