#[derive(Serialize, Deserialize)]
pub struct Config {
    pub default_port: u16,
    pub max_connections: usize,
    pub message_buffer_size: usize,
}