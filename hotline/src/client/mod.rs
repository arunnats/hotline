use crate::shared::*;
use colored::*;

pub fn run() -> Result<()> {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut stdout = io::stdout();

        // Create BufReader from stdin for async input
        let stdin = BufReader::new(io::stdin());
        let mut lines = stdin.lines();

        // Ask for server address
        stdout
            .write_all(b"Enter server address (IP or domain): ")
            .await?;
        stdout.flush().await?;
        let server_addr: String = lines.next_line().await?.unwrap_or_default();

        // Ask for port
        stdout
            .write_all(b"Enter server port (default 8080): ")
            .await?;
        stdout.flush().await?;
        let port_input: String = lines.next_line().await?.unwrap_or_default();
        let port: String = if port_input.trim().is_empty() {
            "8080".to_string()
        } else {
            port_input.trim().to_string()
        };

        // Ask for username
        stdout
            .write_all(b"Enter an optional username (press Enter to skip): ")
            .await?;
        stdout.flush().await?;
        let username: String = lines
            .next_line()
            .await?
            .unwrap_or_default()
            .trim()
            .to_string();

        let address: String = format!("{}:{}", server_addr.trim(), port);
        println!("Connecting to {}...", address);

        let stream: TcpStream = TcpStream::connect(&address).await?;
        let my_socket_addr: SocketAddr = stream.local_addr()?; // <--- This is important
        let my_addr_str = my_socket_addr.to_string(); // Save to compare against `sender`

        println!("{}", format!("Connected as {}", my_addr_str).blue());

        let (reader, writer) = stream.into_split();
        let mut server_reader = BufReader::new(reader);
        let mut server_writer = BufWriter::new(writer);

        // Send username (even if empty)
        server_writer
            .write_all(format!("/username:{}\n", username).as_bytes())
            .await?;
        server_writer.flush().await?;

        // Task to handle reading from server
        tokio::spawn({
            let my_addr_str = my_addr_str.clone();
            async move {
                let mut line = String::new();
                loop {
                    match server_reader.read_line(&mut line).await {
                        Ok(0) => {
                            println!("{}", "\nServer closed the connection.".blue());
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim();

                            if trimmed.starts_with('{') {
                                if let Ok(msg) = serde_json::from_str::<Message>(trimmed) {
                                    if msg.sender == my_addr_str {
                                        line.clear();
                                        continue; // Skip own message
                                    }
                                    let local_time =
                                        msg.timestamp.with_timezone(&Local).format("%H:%M:%S");
                                    let sender_name = msg.username.unwrap_or(msg.sender);

                                    println!(
                                        "[{}] {}: {}",
                                        local_time.to_string().yellow(),
                                        sender_name.green().bold(),
                                        msg.content.cyan()
                                    );
                                } else {
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
            }
        });

        println!(
            "{}",
            "You can now chat! Type and press Enter. Type `/quit` to exit.".blue()
        );

        let mut input_lines = lines; // Continue using same stdin
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
                        println!(
                            "You are on timeout for {:.1} more seconds",
                            remaining.as_secs_f32()
                        );
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
                    println!("You are sending messages too fast! Timeout for 10 seconds.");
                    timeout_until = Some(Instant::now() + Duration::from_secs(10));
                    continue;
                }

                let timestamp = Local::now().format("%H:%M:%S").to_string();
                println!(
                    "[{}] {} {}",
                    timestamp.yellow(),
                    "You:".green().bold(),
                    trimmed.cyan()
                );

                server_writer
                    .write_all(format!("{}\n", trimmed).as_bytes())
                    .await?;
                server_writer.flush().await?;
            } else {
                break;
            }
        }

        Ok(())
    })
}
