use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use chrono::{DateTime, Local, Utc};
use anyhow::{Result, Context};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct Message {
    content: String,
    sender: String,
    timestamp: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "0.0.0.0:8080";
    println!("Starting server on {}", addr);
    
    let listener = TcpListener::bind(addr)
        .await
        .context("Failed to bind to address")?;
    
    let (tx, _) = broadcast::channel::<Message>(100);
    let usernames = Arc::new(Mutex::new(HashMap::<SocketAddr, String>::new()));

    println!("Server running! Waiting for connections...");

    loop {
        let (socket, addr) = listener
            .accept()
            .await
            .context("Failed to accept connection")?;

        println!("New client connected: {}", addr);

        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let usernames = Arc::clone(&usernames);

        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            let welcome = format!("Connected to chat server. Your address is: {}\n", addr);
            writer.write_all(welcome.as_bytes()).await?;

            loop {
                tokio::select! {
                    result = reader.read_line(&mut line) => {
                        if result.context("Failed to read line from client")? == 0 {
                            println!("Client disconnected: {}", addr);
                            usernames.lock().await.remove(&addr);
                            break;
                        }

                        let trimmed = line.trim();

                        // Handle /username:<name> command
                        if trimmed.starts_with("/username:") {
                            let name = trimmed
                                .strip_prefix("/username:")
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            if !name.is_empty() {
                                usernames.lock().await.insert(addr, name.clone());
                                writer.write_all(format!("Username set to '{}'\n", name).as_bytes()).await?;
                            } else {
                                writer.write_all(b"Invalid username command\n").await?;
                            }
                        } else if !trimmed.is_empty() {
                            let usernames = usernames.lock().await;
                            let sender_name = usernames.get(&addr).cloned().unwrap_or(addr.to_string());

                            let msg = Message {
                                content: line.clone(),
                                sender: sender_name,
                                timestamp: Utc::now(),
                            };

                            if let Err(e) = tx.send(msg) {
                                eprintln!("Failed to broadcast message: {}", e);
                            }
                        }

                        line.clear();
                    }
                    result = rx.recv() => {
                        match result {
                            Ok(msg) => {
                                let formatted = format!(
                                    "[{}] {}: {}",
                                    msg.timestamp.with_timezone(&Local).format("%H:%M:%S"),
                                    msg.sender,
                                    msg.content
                                );
                                writer.write_all(formatted.as_bytes()).await?;
                            }
                            Err(e) => {
                                eprintln!("Failed to receive from broadcast: {}", e);
                                break;
                            }
                        }
                    }
                }
            }

            Ok::<_, anyhow::Error>(())
        });
    }
}