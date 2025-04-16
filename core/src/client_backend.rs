use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::serializable_colours::*;
use crate::types::{ChatMessage, OutputEvent, SystemEvent, TextLine};

// Use the shared types from the types module
#[derive(Debug, Clone, Deserialize)]
struct ServerMessage {
    content: String,
    sender: String,
    username: Option<String>,
    timestamp: DateTime<Utc>,
}

pub async fn run_client_backend(
    mut input_rx: mpsc::Receiver<String>,
    output_tx: mpsc::Sender<OutputEvent>,
) -> Result<()> {
    // Ask for server address - send a prompt event
    output_tx
        .send(OutputEvent::SystemEvent(SystemEvent::PromptInput {
            prompt: "Enter server address (IP or domain):".to_string(),
        }))
        .await?;

    // Wait for server address from UI
    let server_addr = if let Some(addr) = input_rx.recv().await {
        addr
    } else {
        return Ok(());
    };

    // Ask for port
    output_tx
        .send(OutputEvent::SystemEvent(SystemEvent::PromptInput {
            prompt: "Enter server port (default 8080):".to_string(),
        }))
        .await?;

    let port_input = if let Some(port) = input_rx.recv().await {
        port
    } else {
        "8080".to_string()
    };

    let port = if port_input.trim().is_empty() {
        "8080".to_string()
    } else {
        port_input.trim().to_string()
    };

    // Ask for username
    output_tx
        .send(OutputEvent::SystemEvent(SystemEvent::PromptInput {
            prompt: "Enter an optional username (press Enter to skip):".to_string(),
        }))
        .await?;

    let username: String = match input_rx.recv().await {
        Some(name) => name.trim().to_string(),
        None => "".to_string(),
    };

    let address: String = format!("{}:{}", server_addr.trim(), port);

    // Send connecting message
    output_tx
        .send(OutputEvent::TextLine(TextLine {
            text: format!("Connecting to {}...", address),
            color: Some(BLUE_COLOR.clone()),
        }))
        .await?;

    // Connect to server
    let stream: TcpStream = match TcpStream::connect(&address).await {
        Ok(s) => s,
        Err(e) => {
            output_tx
                .send(OutputEvent::SystemEvent(SystemEvent::ConnectionError {
                    message: format!("Failed to connect: {}", e),
                }))
                .await?;
            return Ok(());
        }
    };

    let my_socket_addr: SocketAddr = stream.local_addr()?;
    let my_addr_str = my_socket_addr.to_string();

    // Send connection established event
    output_tx
        .send(OutputEvent::SystemEvent(
            SystemEvent::ConnectionEstablished {
                address: my_addr_str.clone(),
            },
        ))
        .await?;

    let (reader, writer) = stream.into_split();
    let mut server_reader = BufReader::new(reader);
    let mut server_writer = BufWriter::new(writer);

    // Send username (even if empty)
    server_writer
        .write_all(format!("/username:{}\n", username).as_bytes())
        .await?;
    server_writer.flush().await?;

    // Task to handle reading from server
    let output_tx_clone = output_tx.clone();
    let my_addr_str_clone = my_addr_str.clone();

    tokio::spawn(async move {
        let mut line = String::new();
        loop {
            match server_reader.read_line(&mut line).await {
                Ok(0) => {
                    let _ = output_tx_clone
                        .send(OutputEvent::SystemEvent(SystemEvent::ConnectionClosed))
                        .await;
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();

                    if trimmed.starts_with('{') {
                        if let Ok(msg) = serde_json::from_str::<ServerMessage>(trimmed) {
                            let is_self = msg.sender == my_addr_str_clone;

                            if !is_self {
                                // Convert to our ChatMessage type
                                let chat_message = ChatMessage {
                                    content: msg.content,
                                    sender: msg.sender,
                                    username: msg.username,
                                    timestamp: msg.timestamp,
                                    is_self: false,
                                };

                                let _ = output_tx_clone
                                    .send(OutputEvent::ChatMessage(chat_message))
                                    .await;
                            }
                        } else {
                            // If it's not a valid message but starts with {, send as text
                            let _ = output_tx_clone
                                .send(OutputEvent::TextLine(TextLine {
                                    text: trimmed.to_string(),
                                    color: None,
                                }))
                                .await;
                        }
                    } else {
                        // Plain text from server
                        let _ = output_tx_clone
                            .send(OutputEvent::TextLine(TextLine {
                                text: trimmed.to_string(),
                                color: None,
                            }))
                            .await;
                    }
                    line.clear();
                }
                Err(e) => {
                    let _ = output_tx_clone
                        .send(OutputEvent::SystemEvent(SystemEvent::ConnectionError {
                            message: format!("Error reading from server: {}", e),
                        }))
                        .await;
                    break;
                }
            }
        }
    });

    // Send welcome message
    output_tx
        .send(OutputEvent::TextLine(TextLine {
            text: "You can now chat! Type and press Enter. Type `/quit` to exit.".to_string(),
            color: Some(GREEN_COLOR.clone()),
        }))
        .await?;

    let mut msg_times: VecDeque<Instant> = VecDeque::new();
    let mut timeout_until: Option<Instant> = None;

    loop {
        // Send prompt
        output_tx
            .send(OutputEvent::TextLine(TextLine {
                text: "> ".to_string(),
                color: None,
            }))
            .await?;

        if let Some(line) = input_rx.recv().await {
            let trimmed = line.trim();

            if trimmed == "/quit" {
                output_tx
                    .send(OutputEvent::TextLine(TextLine {
                        text: "Exiting chat.".to_string(),
                        color: Some(YELLOW_COLOR.clone()),
                    }))
                    .await?;
                break;
            }

            if trimmed.is_empty() {
                continue;
            }

            let now = Instant::now();

            if let Some(timeout) = timeout_until {
                if now < timeout {
                    let remaining = timeout.duration_since(now);
                    output_tx
                        .send(OutputEvent::SystemEvent(SystemEvent::RateLimit {
                            seconds: remaining.as_secs_f32(),
                        }))
                        .await?;
                    continue;
                } else {
                    timeout_until = None;
                }
            }

            // Track messages within 5 seconds window
            msg_times.push_back(now);
            while msg_times
                .front()
                .map_or(false, |t| now.duration_since(*t) > Duration::from_secs(5))
            {
                msg_times.pop_front();
            }

            if msg_times.len() > 10 {
                output_tx
                    .send(OutputEvent::TextLine(TextLine {
                        text: "You are sending messages too fast! Timeout for 10 seconds."
                            .to_string(),
                        color: Some(RED_COLOR.clone()),
                    }))
                    .await?;
                timeout_until = Some(Instant::now() + Duration::from_secs(10));
                continue;
            }

            // Send self message to UI
            let timestamp = Utc::now();
            output_tx
                .send(OutputEvent::ChatMessage(ChatMessage {
                    content: trimmed.to_string(),
                    sender: my_addr_str.clone(),
                    username: Some("You".to_string()),
                    timestamp,
                    is_self: true,
                }))
                .await?;

            // Send message to server
            server_writer
                .write_all(format!("{}\n", trimmed).as_bytes())
                .await?;
            server_writer.flush().await?;
        } else {
            break;
        }
    }

    Ok(())
}
