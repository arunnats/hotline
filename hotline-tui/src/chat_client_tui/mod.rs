mod imports;
mod utils;

pub use imports::*;
pub use utils::*;

pub fn run_chat_tui() {
    // Create standard synchronous channels for UI to communicate with the async thread
    let (ui_to_async_tx, ui_to_async_rx) = std_mpsc::channel::<String>();
    let (async_to_ui_tx, async_to_ui_rx) = std_mpsc::channel::<OutputEvent>();

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
            tokio::spawn(async move {
                while let Ok(message) = ui_to_async_rx.recv() {
                    if input_tx_clone.send(message).await.is_err() {
                        break;
                    }
                }
            });

            // Thread for forwarding backend outputs to UI
            tokio::spawn(async move {
                while let Some(event) = output_rx.recv().await {
                    if async_to_ui_tx.send(event).is_err() {
                        break;
                    }
                }
            });

            // Run the client backend
            if let Err(e) = run_client_backend(input_rx, output_tx).await {
                eprintln!("Backend error: {}", e);
            }
        });
    });

    // Run the UI in the main thread
    chat_tui(ui_to_async_tx, async_to_ui_rx);

    // Wait for the async thread to finish
    let _ = async_thread.join();
}

fn chat_tui(input_tx: std_mpsc::Sender<String>, output_rx: std_mpsc::Receiver<OutputEvent>) {
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

    print_textline_to_output(
        &siv_sink,
        &content,
        TextLine {
            text: "Please follow the prompts to connect to a server.".to_string(),
            color: Some(WHITE_COLOR.clone()),
        },
    );

    let messages = TextView::new_with_content(content.clone())
        .scrollable()
        .full_height()
        .fixed_height(20);

    let input_label = TextView::new("Enter message (type '/quit' to exit)").h_align(HAlign::Left);

    // Use a standard channel sender in the UI callback - NO TOKIO HERE
    let input_tx_clone = input_tx.clone();
    let input = EditView::new()
        .on_submit(move |s, text| {
            // Use synchronous send - no runtime needed
            let _ = input_tx_clone.send(text.to_string());

            s.call_on_name("input", |view: &mut EditView| {
                view.set_content("");
            });
        })
        .with_name("input")
        .fixed_height(1);

    let layout = LinearLayout::vertical()
        .child(messages)
        .child(input_label)
        .child(input.full_width());

    siv.add_layer(Dialog::around(layout).title("Hotline Chat"));

    siv.add_global_callback('q', |s| s.quit());

    // Spawn a thread to handle output events
    let siv_sink_clone = siv.cb_sink().clone();
    thread::spawn(move || {
        while let Ok(event) = output_rx.recv() {
            match event {
                OutputEvent::TextLine(line) => {
                    print_textline_to_output(&siv_sink_clone, &content_clone, line);
                }
                OutputEvent::ChatMessage(msg) => {
                    print_chat_message_to_output(&siv_sink_clone, &content_clone, msg);
                }
                OutputEvent::SystemEvent(event) => {
                    handle_system_event(&siv_sink_clone, &content_clone, event);
                }
            }
        }
    });

    siv.run();
}

fn handle_system_event(siv_sink: &CbSink, content: &TextContent, event: SystemEvent) {
    let content = content.clone();
    let sink = siv_sink.clone();

    sink.send(Box::new(move |_| {
        let mut styled = StyledString::new();

        match event {
            SystemEvent::ConnectionEstablished { address } => {
                styled.append_styled(
                    format!("Connected as {}\n", address),
                    Color::Light(BaseColor::Green),
                );
            }
            SystemEvent::ConnectionClosed => {
                styled.append_styled(
                    "Server closed the connection.\n",
                    Color::Light(BaseColor::Red),
                );
            }
            SystemEvent::ConnectionError { message } => {
                styled.append_styled(
                    format!("Error: {}\n", message),
                    Color::Light(BaseColor::Red),
                );
            }
            SystemEvent::PromptInput { prompt } => {
                styled.append_styled(format!("{}\n", prompt), Color::Light(BaseColor::Magenta));
            }
            SystemEvent::RateLimit { seconds } => {
                styled.append_styled(
                    format!("You are on timeout for {:.1} more seconds\n", seconds),
                    Color::Light(BaseColor::Red),
                );
            }
        }

        content.append(styled);
    }))
    .unwrap();
}
