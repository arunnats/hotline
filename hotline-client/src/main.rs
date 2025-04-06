use tokio::net::TcpStream;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use chrono::{Local, Utc, DateTime};
use anyhow::Result;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    let mut stdout = io::stdout();

    // Create BufReader from stdin for async input
    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    // Ask for server address
    stdout.write_all(b"Enter server address (IP or domain): ").await?;
    stdout.flush().await?;
    let server_addr = lines.next_line().await?.unwrap_or_default();

    // Ask for port
    stdout.write_all(b"Enter server port (default 8080): ").await?;
    stdout.flush().await?;
    let port_input = lines.next_line().await?.unwrap_or_default();
    let port = if port_input.trim().is_empty() {
        "8080".to_string()
    } else {
        port_input.trim().to_string()
    };

    // Ask for username
    stdout.write_all(b"Enter an optional username (press Enter to skip): ").await?;
    stdout.flush().await?;
    let username = lines.next_line().await?.unwrap_or_default().trim().to_string();
    let my_username = username.clone();
    
    let address = format!("{}:{}", server_addr.trim(), port);
    println!("Connecting to {}...", address);

    let stream = TcpStream::connect(&address).await?;
    println!("Connected successfully!");

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
                    println!("\nServer closed the connection.");
                    break;
                }
                Ok(_) => {
                    print!("{}", line);
                    line.clear();
                }
                Err(e) => {
                    eprintln!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    println!("Chat session started. Type your messages and press Enter to send. Type `/quit` to disconnect.");

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
                println!("Disconnecting from server...");
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
            println!("[{}] You: {}", timestamp, trimmed);

            server_writer.write_all(format!("{}\n", trimmed).as_bytes()).await?;
            server_writer.flush().await?;
        } else {
            break;
        }
    }
    Ok(())
}
