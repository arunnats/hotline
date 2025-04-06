use tokio::net::TcpStream;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use chrono::Local;
use anyhow::Result;

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

    let address = format!("{}:{}", server_addr.trim(), port);
    println!("Connecting to {}...", address);

    let stream = TcpStream::connect(&address).await?;
    println!("Connected successfully!");

    let (reader, writer) = stream.into_split();
    let mut server_reader = BufReader::new(reader);
    let mut server_writer = BufWriter::new(writer);

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

    loop {
        stdout.write_all(b"> ").await?;
        stdout.flush().await?;
        if let Some(line) = input_lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed == "/quit" {
                println!("Disconnecting from server...");
                break;
            }

            let timestamp = Local::now().format("%H:%M:%S").to_string();
            println!("[{}] You: {}", timestamp, line);

            server_writer.write_all(format!("{}\n", line).as_bytes()).await?;
            server_writer.flush().await?;
        } else {
            break;
        }
    }

    Ok(())
}
