use tokio::net::TcpListener;
use tokio::sync::{ broadcast, Mutex };
use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, BufReader };
use chrono::{ DateTime, Utc };
use anyhow::{ Result, Context };
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use serde::{ Serialize, Deserialize };

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Message {
    content: String,
    sender: String,
    username: Option<String>,
    timestamp: DateTime<Utc>,
}

#[tokio::main]
pub async fn run_server_backend() -> Result<()> {
    let addr = "0.0.0.0:8080";
    println!("Starting server on {}", addr);

    let listener: TcpListener = TcpListener::bind(addr).await.context("Failed to bind to address")?;

    let (tx, _) = broadcast::channel::<Message>(100);
    let usernames = Arc::new(Mutex::new(HashMap::<SocketAddr, String>::new()));

    println!("Server running! Waiting for connections...");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New client connected: {}", addr);

        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let usernames = Arc::clone(&usernames);

        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            writer.write_all(
                format!("{{\"info\": \"Connected to chat server as {}\"}}\n", addr).as_bytes()
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
                                let _ = tx.send(leave_msg);
                            } else {
                                println!("Client disconnected: {}", addr);
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
                                let _ = tx.send(join_msg);
                            } else {
                                writer.write_all(b"{\"error\": \"Invalid username command\"}\n").await?;
                            }
                        } else if !trimmed.is_empty() {
                            let usernames = usernames.lock().await;
                            let sender = addr.to_string();
                            let username = usernames.get(&addr).cloned();

                            let msg = Message {
                                content: trimmed.to_string(),
                                sender,
                                username,
                                timestamp: Utc::now(),
                            };

                            if let Err(e) = tx.send(msg) {
                                eprintln!("Broadcast failed: {}", e);
                            }
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
}