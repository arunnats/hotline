use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast, mpsc};

use crate::types::{ChatMessage, OutputEvent, TextLine};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    content: String,
    sender: String,
    username: Option<String>,
    timestamp: DateTime<Utc>,
}

pub async fn run_server_backend(
    mut input_rx: mpsc::Receiver<String>,
    output_tx: mpsc::Sender<OutputEvent>,
    shutdown_signal: Arc<AtomicBool>,
) -> Result<()> {
    let mut server_config = None;
    let mut log_file = None;

    // Wait for server configuration
    while let Some(input) = input_rx.recv().await {
        if input.starts_with("START:") {
            let parts: Vec<&str> = input.split(':').collect();
            if parts.len() >= 4 {
                let chatroom = parts[1].to_string();
                let port = parts[2].parse::<u16>().unwrap_or(8080);
                let logging = parts[3].to_lowercase() == "yes";

                // Set up logging if enabled
                if logging {
                    let filename = format!(
                        "chatroom_{}_{}.log",
                        chatroom,
                        Utc::now().format("%Y%m%d_%H%M%S")
                    );
                    if let Ok(file) = File::create(&filename) {
                        log_file = Some(file);
                        let _ = output_tx
                            .send(OutputEvent::TextLine(TextLine {
                                text: format!("Logging enabled. Log file: {}", filename),
                                color: None,
                            }))
                            .await;
                    }
                }

                server_config = Some((chatroom, port));
                break;
            }
        }
    }

    let (chatroom, port) =
        server_config.ok_or_else(|| anyhow::anyhow!("No server configuration received"))?;
    let addr = format!("0.0.0.0:{}", port);

    let _ = output_tx
        .send(OutputEvent::TextLine(TextLine {
            text: format!("Starting server for chatroom '{}' on {}", chatroom, addr),
            color: None,
        }))
        .await;

    let listener: TcpListener = TcpListener::bind(&addr)
        .await
        .context("Failed to bind to address")?;

    let (tx, _) = broadcast::channel::<Message>(100);
    let usernames = Arc::new(Mutex::new(HashMap::<SocketAddr, String>::new()));

    let _ = output_tx
        .send(OutputEvent::TextLine(TextLine {
            text: "Server running! Waiting for connections...".to_string(),
            color: None,
        }))
        .await;

    loop {
        if shutdown_signal.load(Ordering::SeqCst) {
            break;
        }

        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((socket, addr)) => {
                        let _ = output_tx.send(OutputEvent::TextLine(TextLine {
                            text: format!("New client connected: {}", addr),
                            color: None,
                        })).await;

                        let tx = tx.clone();
                        let mut rx = tx.subscribe();
                        let usernames = Arc::clone(&usernames);
                        let output_tx = output_tx.clone();
                        let chatroom = chatroom.clone();
                        // let log_file = Arc::new(Mutex::new(log_file.clone()));

                        tokio::spawn(async move {
                            let (reader, mut writer) = socket.into_split();
                            let mut reader = BufReader::new(reader);
                            let mut line = String::new();

                            writer.write_all(
                                format!("{{\"info\": \"Connected to chatroom '{}'\"}}\n", chatroom).as_bytes()
                            ).await?;

                            loop {
                                tokio::select! {
                                    result = reader.read_line(&mut line) => {
                                        if result? == 0 {
                                            // Client disconnected
                                            let mut usernames_lock = usernames.lock().await;
                                            if let Some(name) = usernames_lock.remove(&addr) {
                                                let leave_msg = Message {
                                                    content: format!("{} has left the chat", name),
                                                    sender: "Server".to_string(),
                                                    username: Some(name.clone()),
                                                    timestamp: Utc::now(),
                                                };
                                                let _ = tx.send(leave_msg.clone());
                                                let _ = output_tx.send(OutputEvent::ChatMessage(ChatMessage {
                                                    content: leave_msg.content.clone(),
                                                    sender: leave_msg.sender.clone(),
                                                    username: leave_msg.username.clone(),
                                                    timestamp: leave_msg.timestamp,
                                                    is_self: false,
                                                })).await;
                                            }
                                            break;
                                        }

                                        let trimmed = line.trim();

                                        if trimmed.starts_with("/username:") {
                                            let name = trimmed.strip_prefix("/username:").unwrap_or("").trim().to_string();
                                            if !name.is_empty() {
                                                let mut usernames_lock = usernames.lock().await;
                                                usernames_lock.insert(addr, name.clone());

                                                writer.write_all(format!("{{\"info\": \"Username set to '{}'\"}}\n", name).as_bytes()).await?;

                                                let join_msg = Message {
                                                    content: format!("{} has joined the chat", name),
                                                    sender: "Server".to_string(),
                                                    username: Some(name.clone()),
                                                    timestamp: Utc::now(),
                                                };
                                                let _ = tx.send(join_msg.clone());
                                                let _ = output_tx.send(OutputEvent::ChatMessage(ChatMessage {
                                                    content: join_msg.content.clone(),
                                                    sender: join_msg.sender.clone(),
                                                    username: join_msg.username.clone(),
                                                    timestamp: join_msg.timestamp,
                                                    is_self: false,
                                                })).await;
                                            } else {
                                                writer.write_all(b"{\"error\": \"Invalid username command\"}\n").await?;
                                            }
                                        } else if !trimmed.is_empty() {
                                            let usernames = usernames.lock().await;
                                            let sender = usernames.get(&addr).cloned().unwrap_or_else(|| addr.to_string());
                                            let username = usernames.get(&addr).cloned();

                                            let msg = Message {
                                                content: trimmed.to_string(),
                                                sender: sender.clone(),
                                                username: username.clone(),
                                                timestamp: Utc::now(),
                                            };

                                            if let Err(e) = tx.send(msg.clone()) {
                                                eprintln!("Broadcast failed: {}", e);
                                            }

                                            let _ = output_tx.send(OutputEvent::ChatMessage(ChatMessage {
                                                content: msg.content.clone(),
                                                sender: sender.clone(),
                                                username: username.clone(),
                                                timestamp: msg.timestamp,
                                                is_self: false,
                                            })).await;

                                            // Log message if logging is enabled
                                            // if let Some(ref mut log) = log_file.lock().await {
                                            //     let _ = writeln!(log, "[{}] {}: {}",
                                            //         msg.timestamp.format("%Y-%m-%d %H:%M:%S"),
                                            //         msg.sender.clone(),
                                            //         msg.content.clone()
                                            //     );
                                            // }
                                        }

                                        line.clear();
                                    }

                                    result = rx.recv() => {
                                        if let Ok(msg) = result {
                                            let json = serde_json::to_string(&msg)?;
                                            writer.write_all(json.as_bytes()).await?;
                                            writer.write_all(b"\n").await?;
                                        }
                                    }
                                }
                            }

                            Ok::<_, anyhow::Error>(())
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                    }
                }
            }

            Some(input) = input_rx.recv() => {
                if input == "/end" {
                    break;
                }

                // Handle server host messages
                let msg = Message {
                    content: input,
                    sender: "Host".to_string(),
                    username: None,
                    timestamp: Utc::now(),
                };

                let tx = tx.clone();
                let output_tx = output_tx.clone();

                if let Err(e) = tx.send(msg.clone()) {
                    eprintln!("Broadcast failed: {}", e);
                }

                let _ = output_tx.send(OutputEvent::ChatMessage(ChatMessage {
                    content: msg.content.clone(),
                    sender: msg.sender.clone(),
                    username: Some("Host".to_string()),
                    timestamp: msg.timestamp,
                    is_self: true,
                })).await;

                // Log message if logging is enabled
                if let Some(ref mut log) = log_file {
                    let _ = writeln!(log, "[{}] {}: {}",
                        msg.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        msg.sender.clone(),
                        msg.content.clone()
                    );
                }
            }
        }
    }

    let _ = output_tx
        .send(OutputEvent::TextLine(TextLine {
            text: "Server shutting down...".to_string(),
            color: None,
        }))
        .await;

    Ok(())
}
