mod imports;
mod utils;

pub use imports::*;
pub use utils::*;

pub fn run_chat_tui() {
    // Create a shared shutdown signal
    let shutdown_signal = Arc::new(AtomicBool::new(false));

    // Create standard synchronous channels for UI to communicate with the async thread
    let (ui_to_async_tx, ui_to_async_rx) = std_mpsc::channel::<String>();
    let (async_to_ui_tx, async_to_ui_rx) = std_mpsc::channel::<OutputEvent>();

    // Clone the shutdown signal for the async thread
    let thread_shutdown_signal = Arc::clone(&shutdown_signal);

    // Spawn the async thread with its own Tokio runtime
    let async_thread: thread::JoinHandle<()> = thread::spawn(move || {
        // Create a new Tokio runtime in this thread
        let rt = Runtime::new().unwrap();

        // Run the async code in this runtime
        rt.block_on(async {
            // Create Tokio channels for the async code
            let (input_tx, input_rx) = mpsc::channel::<String>(100);
            let (output_tx, mut output_rx) = mpsc::channel::<OutputEvent>(100);

            // Bridge between std_mpsc and tokio channels

            // Thread for forwarding UI inputs to async backend
            let input_tx_clone = input_tx.clone();
            let input_shutdown = Arc::clone(&thread_shutdown_signal);
            let input_handle = tokio::spawn(async move {
                while let Ok(message) = ui_to_async_rx.recv() {
                    // Check if we should shut down
                    if input_shutdown.load(Ordering::SeqCst) {
                        break;
                    }

                    if input_tx_clone.send(message).await.is_err() {
                        break;
                    }
                }
            });

            // Thread for forwarding backend outputs to UI
            let output_shutdown = Arc::clone(&thread_shutdown_signal);
            let output_handle = tokio::spawn(async move {
                while let Some(event) = output_rx.recv().await {
                    // Check if we should shut down
                    if output_shutdown.load(Ordering::SeqCst) {
                        break;
                    }

                    if async_to_ui_tx.send(event).is_err() {
                        break;
                    }
                }
            });

            // Run the client backend
            if let Err(e) = run_client_backend(input_rx, output_tx, thread_shutdown_signal).await {
                eprintln!("Backend error: {}", e);
            }

            // Wait for all spawned tasks to complete
            let _ = input_handle.await;
            let _ = output_handle.await;
        });
    });

    // Run the UI in the main thread
    chat_tui(ui_to_async_tx, async_to_ui_rx, shutdown_signal);

    // Wait for the async thread to finish
    let _ = async_thread.join();
}

fn show_connection_dialog(
    siv: &mut Cursive,
    input_tx: std_mpsc::Sender<String>,
    content: TextContent,
    quit_signal: Arc<AtomicBool>,
) {
    // Create input fields for server address and port
    let server_input = EditView::new().with_name("server_addr").fixed_width(30);

    let port_input = EditView::new()
        .content("8080")
        .with_name("port")
        .fixed_width(10);

    let username_input = EditView::new().with_name("username").fixed_width(30);

    // Create the layout for the dialog
    let layout = LinearLayout::vertical()
        .child(TextView::new("Server Address:"))
        .child(server_input)
        .child(TextView::new("Port:"))
        .child(port_input)
        .child(TextView::new("Username (optional):"))
        .child(username_input);

    // Create the dialog with buttons
    let dialog = Dialog::around(layout)
        .title("Connect to Server")
        .button("Connect", move |s| {
            // Get values from input fields
            let server_addr = s
                .call_on_name("server_addr", |view: &mut EditView| {
                    view.get_content().to_string()
                })
                .unwrap_or_default();

            let port = s
                .call_on_name("port", |view: &mut EditView| view.get_content().to_string())
                .unwrap_or("8080".to_string());

            let username = s
                .call_on_name("username", |view: &mut EditView| {
                    view.get_content().to_string()
                })
                .unwrap_or_default();

            if server_addr.trim().is_empty() {
                s.add_layer(Dialog::info("Please enter a server address").title("Error"));
                return;
            }

            // Remove the dialog
            s.pop_layer();

            // Show connecting message
            let content_clone = content.clone();
            s.call_on_name("messages", |view: &mut TextView| {
                let mut styled = StyledString::new();
                styled.append_styled(
                    format!("Connecting to {}:{}...\n", server_addr, port),
                    Color::Light(BaseColor::Blue),
                );
                content_clone.append(styled);
            });

            // Send connection details to backend
            let _ = input_tx.send(format!("CONNECT:{}:{}:{}", server_addr, port, username));
        })
        .button("Quit", move |s| {
            let cb_sink = s.cb_sink().clone();
            global_quit(&cb_sink, &quit_signal);
        });

    siv.add_layer(dialog);
}

fn chat_tui(
    input_tx: std_mpsc::Sender<String>,
    output_rx: std_mpsc::Receiver<OutputEvent>,
    shutdown_signal: Arc<AtomicBool>,
) {
    let mut siv = cursive::default();
    set_custom_theme(&mut siv);

    let content = TextContent::new("");
    let content_clone = content.clone();
    let siv_sink = siv.cb_sink().clone();

    // Add welcome messages from the frontend
    print_textline_to_output(
        &siv_sink,
        &content,
        TextLine {
            text: "Welcome to Hotline Chat!".to_string(),
            color: Some(BLUE_COLOR.clone()),
        },
    );

    let messages = TextView::new_with_content(content.clone())
        .scrollable()
        .full_height()
        .fixed_height(20);

    let input_label = TextView::new("Enter message (type '/quit' to exit)").h_align(HAlign::Left);

    // Use a standard channel sender in the UI callback - NO TOKIO HERE
    let input_tx_clone = input_tx.clone();
    let quit_signal = Arc::clone(&shutdown_signal);

    let input = EditView::new()
        .on_submit(move |s, text| {
            if text != "/quit" {
                let _ = input_tx_clone.send(text.to_string());

                s.call_on_name("input", |view: &mut EditView| {
                    view.set_content("");
                });
            } else {
                // Set the shutdown signal before quitting the UI
                global_quit(&s.cb_sink().clone(), &quit_signal);
            }
        })
        .with_name("input")
        .fixed_height(1);

    let layout = LinearLayout::vertical()
        .child(messages)
        .child(input_label)
        .child(input.full_width());

    siv.add_layer(Dialog::around(layout).title("Hotline Chat"));

    // Show connection dialog at startup
    let dialog_input_tx = input_tx.clone();
    let dialog_content = content.clone();

    siv.add_layer(
        Dialog::text("Would you like to connect to a server?")
            .title("Connection")
            .button("Connect", {
                let shutdown_signal = shutdown_signal.clone(); // clone for this closure
                let dialog_input_tx = dialog_input_tx.clone();
                let dialog_content = dialog_content.clone();
                move |s| {
                    let dialog_input_tx = dialog_input_tx.clone();
                    let dialog_content = dialog_content.clone();
                    let shutdown_signal = shutdown_signal.clone();
                    s.pop_layer();
                    show_connection_dialog(s, dialog_input_tx, dialog_content, shutdown_signal);
                }
            })
            .button("Quit", |s| s.quit()),
    );

    // Spawn a thread to handle output events
    let siv_sink_clone = siv.cb_sink().clone();
    let output_shutdown = shutdown_signal.clone();
    let shutdown_signal_for_thread = shutdown_signal.clone(); // clone again for system event handler

    let output_thread = thread::spawn(move || {
        while let Ok(event) = output_rx.recv() {
            if output_shutdown.load(Ordering::SeqCst) {
                break;
            }

            match event {
                OutputEvent::TextLine(line) => {
                    print_textline_to_output(&siv_sink_clone, &content_clone, line);
                }
                OutputEvent::ChatMessage(msg) => {
                    print_chat_message_to_output(&siv_sink_clone, &content_clone, msg);
                }
                OutputEvent::SystemEvent(event) => {
                    handle_system_event(
                        &siv_sink_clone,
                        &content_clone,
                        event,
                        input_tx.clone(),
                        shutdown_signal_for_thread.clone(), // clone here
                    );
                }
            }
        }
    });

    // Run the UI
    siv.run();

    // After UI exits, ensure shutdown signal is set
    shutdown_signal.store(true, Ordering::SeqCst);

    // Wait for the output thread to finish
    let _ = output_thread.join();

    let _ = std::thread::spawn(move || {
        // Give threads a short time to clean up
        std::thread::sleep(Duration::from_millis(400));
        // Force exit the process
        std::process::exit(0);
    });
}

fn handle_system_event(
    siv_sink: &CbSink,
    content: &TextContent,
    event: SystemEvent,
    input_tx: std_mpsc::Sender<String>,
    shutdown_signal: Arc<AtomicBool>,
) {
    let content = content.clone();
    let sink = siv_sink.clone();
    let input_tx = input_tx.clone();

    sink.send(Box::new(move |s| {
        let mut styled = StyledString::new();

        match event {
            SystemEvent::ConnectionEstablished { address } => {
                styled.append_styled(
                    format!("Connected as {}\n", address),
                    Color::Light(BaseColor::Green),
                );
                content.append(styled);
            }
            SystemEvent::ConnectionClosed => {
                styled.append_styled(
                    "Server closed the connection.\n",
                    Color::Light(BaseColor::Red),
                );
                content.append(styled);

                // Show connection dialog again
                show_connection_dialog(
                    s,
                    input_tx.clone(),
                    content.clone(),
                    shutdown_signal.clone(),
                );
            }
            SystemEvent::ConnectionError { message } => {
                styled.append_styled(
                    format!("Error: {}\n", message),
                    Color::Light(BaseColor::Red),
                );
                content.append(styled);

                // Show connection dialog again
                show_connection_dialog(
                    s,
                    input_tx.clone(),
                    content.clone(),
                    shutdown_signal.clone(),
                );
            }
            SystemEvent::PromptInput { prompt } => {
                styled.append_styled(format!("{}\n", prompt), Color::Light(BaseColor::Magenta));
                content.append(styled);
            }
            SystemEvent::RateLimit { seconds } => {
                styled.append_styled(
                    format!("You are on timeout for {:.1} more seconds\n", seconds),
                    Color::Light(BaseColor::Red),
                );
                content.append(styled);
            }
        }
    }))
    .unwrap();
}
