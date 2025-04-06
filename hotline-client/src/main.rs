use tokio::net::TcpStream;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use chrono::{DateTime, Local, Utc};
use anyhow::Result;
use serde::Deserialize;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use colored::*;

#[derive(Deserialize)]
struct Message {
    content: String,
    sender: String,
    timestamp: DateTime<Utc>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut stdout = io::stdout();

    // Create BufReader from stdin for async input
    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    // Ask for server address
    stdout.write_all(b"Enter server address (IP or domain): ").await?;
    stdout.flush().await?;
    let server_addr: String = lines.next_line().await?.unwrap_or_default();

    // Ask for port
    stdout.write_all(b"Enter server port (default 8080): ").await?;
    stdout.flush().await?;
    let port_input: String = lines.next_line().await?.unwrap_or_default();
    let port: String = if port_input.trim().is_empty() {
        "8080".to_string()
    } else {
        port_input.trim().to_string()
    };

    // Ask for username
    stdout.write_all(b"Enter an optional username (press Enter to skip): ").await?;
    stdout.flush().await?;
    let username: String = lines.next_line().await?.unwrap_or_default().trim().to_string();
    let my_username: String = username.clone();
    
    let address: String = format!("{}:{}", server_addr.trim(), port);
    println!("Connecting to {}...", address);

    let stream: TcpStream = TcpStream::connect(&address).await?;
    println!("{}","Connected successfully".blue());

    let (reader, writer) = stream.into_split();
    let mut server_reader = BufReader::new(reader);
    let mut server_writer = BufWriter::new(writer);

    // Send username (even if empty)
    server_writer.write_all(format!("/username:{}\n", username).as_bytes()).await?;
    server_writer.flush().await?;

    // Spawn task for reading from server
    tokio::spawn(async move {
        let mut line = String::new();
        loop {
            match server_reader.read_line(&mut line).await {
                Ok(0) => {
                    println!("{}","\nServer closed the connection.".blue());
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.starts_with('{') {
                        if let Ok(msg) = serde_json::from_str::<Message>(trimmed) {
                            if msg.sender == my_username {
                                line.clear();
                                continue; // skip own message
                            }
                            let local_time = msg.timestamp.with_timezone(&Local).format("%H:%M:%S");
                            println!("[{}] {}: {}", local_time.to_string().yellow(), msg.sender.green().bold(), msg.content.cyan());
                        } else {
                            // fallback if it's a simple info/error message
                            println!("{}", trimmed.blue());
                        }
                    } else {
                        println!("{}", trimmed);
                    }
                    line.clear();
                }
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    println!("{}","You can now chat! Type and press Enter. Type `/quit` to exit.".blue());

    let stdin = BufReader::new(io::stdin());
    let mut input_lines = stdin.lines();
    let mut msg_times: VecDeque<Instant> = VecDeque::new();
    let mut timeout_until: Option<Instant> = None;

    loop {
        stdout.write_all(b"> ").await?;
        stdout.flush().await?;

        if let Some(line) = input_lines.next_line().await? {
            let trimmed = line.trim();

            if trimmed == "/quit" {
                println!("Exiting chat.");
                break;
            }

            if trimmed.is_empty() {
                continue;
            }

            let now = Instant::now();

            if let Some(timeout) = timeout_until {
                if now < timeout {
                    let remaining = timeout.duration_since(now);
                    println!("You are on timeout for {:.1} more seconds", remaining.as_secs_f32());
                    continue;
                } else {
                    timeout_until = None;
                }
            }

            // Track messages within 5 seconds window
            msg_times.push_back(now);
            while msg_times.front().map_or(false, |t| now.duration_since(*t) > Duration::from_secs(5)) {
                msg_times.pop_front();
            }

            if msg_times.len() > 10 {
                println!("You are sending messages too fast! You are in timeout for 10 seconds.");
                timeout_until = Some(Instant::now() + Duration::from_secs(10));
                continue;
            }

            let timestamp = Local::now().format("%H:%M:%S").to_string();
            println!("[{}] {} {}", timestamp.yellow(), "You:".green().bold(), trimmed.cyan());

            server_writer.write_all(format!("{}\n", trimmed).as_bytes()).await?;
            server_writer.flush().await?;
        } else {
            break;
        }
    }
    Ok(())
}