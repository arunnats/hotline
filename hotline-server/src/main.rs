use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use chrono::{DateTime, Local, Utc};
use anyhow::{Result, Context};

#[derive(Clone, Debug)]
struct Message {
    content: String,
    sender: std::net::SocketAddr,
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

    println!("Server running! Waiting for connections...");

    loop {
        let (socket, addr) = listener
            .accept()
            .await
            .context("Failed to accept connection")?;

        println!("New client connected: {}", addr);

        let tx = tx.clone();
        let mut rx = tx.subscribe();

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
                            break;
                        }

                        let msg = Message {
                            content: line.clone(),
                            sender: addr,
                            timestamp: Utc::now(),
                        };

                        if let Err(e) = tx.send(msg) {
                            eprintln!("Failed to broadcast message: {}", e);
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
