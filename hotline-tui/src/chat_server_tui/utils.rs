use super::imports::*;
use super::restart_server_tui;

pub fn show_server_setup_dialog(
    siv: &mut Cursive,
    input_tx: std_mpsc::Sender<String>,
    content: TextContent,
    quit_signal: Arc<AtomicBool>,
) {
    // Clear existing content before showing setup
    content.set_content("");

    // Create input fields for server configuration
    let chatroom_input = EditView::new().with_name("chatroom").fixed_width(30);
    let port_input = EditView::new()
        .content("8080")
        .with_name("port")
        .fixed_width(10);
    let logging_input = EditView::new()
        .content("yes")
        .with_name("logging")
        .fixed_width(5);

    // Create the layout for the dialog
    let layout = LinearLayout::vertical()
        .child(TextView::new("Chatroom Name:"))
        .child(chatroom_input)
        .child(TextView::new("Port:"))
        .child(port_input)
        .child(TextView::new("Enable Logging (yes/no):"))
        .child(logging_input);

    // Create the dialog with buttons
    let dialog = Dialog::around(layout)
        .title("Server Setup")
        .button("Start Server", move |s| {
            // Get values from input fields
            let chatroom = s
                .call_on_name("chatroom", |view: &mut EditView| {
                    view.get_content().to_string()
                })
                .unwrap_or_default();

            let port = s
                .call_on_name("port", |view: &mut EditView| view.get_content().to_string())
                .unwrap_or("8080".to_string());

            let logging = s
                .call_on_name("logging", |view: &mut EditView| {
                    view.get_content().to_string()
                })
                .unwrap_or("yes".to_string());

            if chatroom.trim().is_empty() {
                s.add_layer(Dialog::info("Please enter a chatroom name").title("Error"));
                return;
            }

            // Try to parse port number
            match port.trim().parse::<u16>() {
                Ok(_) => {} // Valid port
                Err(_) => {
                    s.add_layer(
                        Dialog::info("Please enter a valid port number (0-65535)").title("Error"),
                    );
                    return;
                }
            }

            // Remove the dialog
            s.pop_layer();

            // Show starting message
            let content_clone = content.clone();
            s.call_on_name("messages", |_view: &mut TextView| {
                let mut styled = StyledString::new();
                styled.append_styled(
                    format!(
                        "Starting server for chatroom '{}' on port {}...\n",
                        chatroom, port
                    ),
                    Color::Light(BaseColor::Blue),
                );
                content_clone.append(styled);
            });

            // Send server configuration to backend
            let _ = input_tx.send(format!("START:{}:{}:{}", chatroom, port, logging));
        })
        .button("Quit", {
            let quit_signal = quit_signal.clone();
            move |s| global_quit(&s.cb_sink().clone(), &quit_signal)
        });

    siv.add_layer(dialog);
}

pub fn server_tui(
    input_tx: std_mpsc::Sender<String>,
    output_rx: std_mpsc::Receiver<OutputEvent>,
    shutdown_signal: Arc<AtomicBool>,
) {
    let mut siv = cursive::default();
    set_custom_theme(&mut siv);

    // Create auto-scroll state
    let auto_scroll = Arc::new(Mutex::new(true));

    // Clone for closures
    let auto_scroll_up = auto_scroll.clone();
    let auto_scroll_pgup = auto_scroll.clone();
    let auto_scroll_down = auto_scroll.clone();

    // Disable auto-scroll when user scrolls up
    siv.add_global_callback(
        cursive::event::Event::Key(cursive::event::Key::Up),
        move |_| {
            if let Ok(mut scroll) = auto_scroll_up.lock() {
                *scroll = false;
            }
        },
    );

    // Disable auto-scroll when user presses Page Up
    siv.add_global_callback(
        cursive::event::Event::Key(cursive::event::Key::PageUp),
        move |_| {
            if let Ok(mut scroll) = auto_scroll_pgup.lock() {
                *scroll = false;
            }
        },
    );

    // Re-enable auto-scroll when user scrolls to bottom
    siv.add_global_callback(
        cursive::event::Event::Key(cursive::event::Key::End),
        move |_| {
            if let Ok(mut scroll) = auto_scroll_down.lock() {
                *scroll = true;
            }
        },
    );

    let content = TextContent::new("");
    let content_clone = content.clone();
    let siv_sink = siv.cb_sink().clone();

    // Add welcome message
    print_textline_to_output(
        &siv_sink,
        &content,
        TextLine {
            text: "Welcome to Hotline Server!".to_string(),
            color: Some(BLUE_COLOR.clone()),
        },
        &auto_scroll,
    );

    let messages = TextView::new_with_content(content.clone())
        .scrollable()
        .with_name("messages_scroll")
        .full_height()
        .fixed_height(20);

    let input_label =
        TextView::new("Enter message (type '/end' to stop server)").h_align(HAlign::Left);

    let input_tx_clone = input_tx.clone();
    let shutdown_signal_clone = shutdown_signal.clone();
    let siv_cb_sink = siv.cb_sink().clone();

    let input = EditView::new()
        .on_submit(move |s, text| {
            if text != "/end" {
                let _ = input_tx_clone.send(text.to_string());
                s.call_on_name("input", |view: &mut EditView| {
                    view.set_content("");
                });
            } else {
                // Send /end command to server
                let _ = input_tx_clone.send("/end".to_string());
                // Call restart_server_tui
                restart_server_tui(shutdown_signal_clone.clone(), siv_cb_sink.clone());
            }
        })
        .with_name("input")
        .fixed_height(1);

    let layout = LinearLayout::vertical()
        .child(messages)
        .child(input_label)
        .child(input.full_width());

    siv.add_layer(Dialog::around(layout).title("Hotline Server"));

    // Spawn a thread to handle output events
    let siv_sink_clone = siv.cb_sink().clone();
    let content_clone_for_thread = content_clone.clone();
    let output_shutdown = shutdown_signal.clone();
    let shutdown_signal_for_thread = shutdown_signal.clone();
    let input_tx_for_thread = input_tx.clone();
    let auto_scroll_for_thread = auto_scroll.clone();
    let siv_cb_sink_thread = siv.cb_sink().clone();

    let output_thread = thread::spawn(move || {
        while !output_shutdown.load(Ordering::SeqCst) {
            match output_rx.recv() {
                Ok(event) => {
                    match event {
                        OutputEvent::TextLine(line) => {
                            print_textline_to_output(
                                &siv_sink_clone,
                                &content_clone_for_thread,
                                line,
                                &auto_scroll_for_thread,
                            );
                        }
                        OutputEvent::ChatMessage(msg) => {
                            print_chat_message_to_output(
                                &siv_sink_clone,
                                &content_clone_for_thread,
                                msg,
                                &auto_scroll_for_thread,
                            );
                        }
                        OutputEvent::SystemEvent(event) => {
                            // Handle system events
                            handle_system_event(
                                &siv_sink_clone,
                                &content_clone_for_thread,
                                event,
                                input_tx_for_thread.clone(),
                                shutdown_signal_for_thread.clone(),
                                &auto_scroll_for_thread,
                                siv_cb_sink_thread.clone(),
                            );
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Show server setup dialog at startup
    show_server_setup_dialog(
        &mut siv,
        input_tx.clone(),
        content.clone(),
        shutdown_signal.clone(),
    );

    // Run the UI
    siv.run();

    // After UI exits, ensure shutdown signal is set
    shutdown_signal.store(true, Ordering::SeqCst);

    // Wait for the output thread to finish
    let _ = output_thread.join();
}

pub fn handle_system_event(
    siv_sink: &CbSink,
    content: &TextContent,
    event: SystemEvent,
    _input_tx: std_mpsc::Sender<String>,
    shutdown_signal: Arc<AtomicBool>,
    auto_scroll: &Arc<Mutex<bool>>,
    siv_cb_sink: cursive::CbSink,
) {
    let content = content.clone();
    let sink = siv_sink.clone();
    let auto_scroll = auto_scroll.clone();
    let shutdown_signal = shutdown_signal.clone();
    let siv_cb_sink = siv_cb_sink.clone();

    sink.send(Box::new(move |s| {
        let mut styled = StyledString::new();

        match event {
            SystemEvent::ConnectionEstablished { address } => {
                styled.append_styled(
                    format!("Client connected: {}\n", address),
                    Color::Light(BaseColor::Green),
                );
                content.append(styled);

                // Auto-scroll if enabled
                if let Ok(scroll) = auto_scroll.lock() {
                    if *scroll {
                        s.call_on_name("messages_scroll", |view: &mut ScrollView<TextView>| {
                            view.scroll_to_bottom();
                        });
                    }
                }
            }
            SystemEvent::ConnectionClosed => {
                styled.append_styled("Client disconnected\n", Color::Light(BaseColor::Yellow));
                content.append(styled);

                // Auto-scroll if enabled
                if let Ok(scroll) = auto_scroll.lock() {
                    if *scroll {
                        s.call_on_name("messages_scroll", |view: &mut ScrollView<TextView>| {
                            view.scroll_to_bottom();
                        });
                    }
                }
            }
            SystemEvent::ConnectionError { message } => {
                styled.append_styled(
                    format!("Error: {}\n", message),
                    Color::Light(BaseColor::Red),
                );
                content.append(styled);

                // Auto-scroll if enabled
                if let Ok(scroll) = auto_scroll.lock() {
                    if *scroll {
                        s.call_on_name("messages_scroll", |view: &mut ScrollView<TextView>| {
                            view.scroll_to_bottom();
                        });
                    }
                }

                // Call restart_server_tui after a brief delay
                let shutdown_signal = shutdown_signal.clone();
                let siv_cb_sink = siv_cb_sink.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    restart_server_tui(shutdown_signal, siv_cb_sink);
                });
            }
            SystemEvent::PromptInput { prompt } => {
                styled.append_styled(format!("{}\n", prompt), Color::Light(BaseColor::Magenta));
                content.append(styled);

                // Auto-scroll if enabled
                if let Ok(scroll) = auto_scroll.lock() {
                    if *scroll {
                        s.call_on_name("messages_scroll", |view: &mut ScrollView<TextView>| {
                            view.scroll_to_bottom();
                        });
                    }
                }
            }
            SystemEvent::RateLimit { .. } => {
                // Rate limit events are handled by the backend, no need to show in UI
            }
        }
    }))
    .unwrap();
}

pub fn print_textline_to_output(
    siv_sink: &CbSink,
    content: &TextContent,
    line: TextLine,
    auto_scroll: &Arc<Mutex<bool>>,
) {
    let content = content.clone();
    let sink = siv_sink.clone();
    let auto_scroll = auto_scroll.clone();

    sink.send(Box::new(move |s| {
        let mut styled = StyledString::new();
        if let Some(color) = line.color {
            // Convert SerializableColor to cursive::style::Color
            let color_converted: cursive::style::Color = color.into();
            styled.append_styled(line.text, color_converted);
        } else {
            styled.append_plain(line.text);
        }
        content.append(styled);

        // Auto-scroll if enabled
        if let Ok(scroll) = auto_scroll.lock() {
            if *scroll {
                s.call_on_name("messages_scroll", |view: &mut ScrollView<TextView>| {
                    view.scroll_to_bottom();
                });
            }
        }
    }))
    .unwrap();
}

pub fn print_chat_message_to_output(
    siv_sink: &CbSink,
    content: &TextContent,
    msg: ChatMessage,
    auto_scroll: &Arc<Mutex<bool>>,
) {
    let content = content.clone();
    let sink = siv_sink.clone();
    let auto_scroll = auto_scroll.clone();

    sink.send(Box::new(move |s| {
        let mut styled = StyledString::new();
        let sender_name = if let Some(username) = &msg.username {
            username.clone()
        } else {
            msg.sender.clone()
        };

        // Format based on whether this is a message from the host or not
        if msg.is_self {
            styled.append_styled(format!("{}: ", sender_name), Color::Light(BaseColor::Blue));
        } else {
            styled.append_styled(format!("{}: ", sender_name), Color::Light(BaseColor::Green));
        }

        styled.append_plain(&msg.content);
        styled.append_plain("\n");
        content.append(styled);

        // Auto-scroll if enabled
        if let Ok(scroll) = auto_scroll.lock() {
            if *scroll {
                s.call_on_name("messages_scroll", |view: &mut ScrollView<TextView>| {
                    view.scroll_to_bottom();
                });
            }
        }
    }))
    .unwrap();
}

pub fn set_custom_theme(siv: &mut cursive::CursiveRunnable) {
    let mut theme = Theme::default();
    let mut palette = Palette::default();

    palette[PaletteColor::Background] = Color::TerminalDefault;
    palette[PaletteColor::View] = Color::TerminalDefault;
    palette[PaletteColor::Primary] = Color::Dark(BaseColor::Blue);
    palette[PaletteColor::Secondary] = Color::Light(BaseColor::Blue);
    palette[PaletteColor::Tertiary] = Color::Light(BaseColor::White);
    palette[PaletteColor::TitlePrimary] = Color::Light(BaseColor::Green);
    palette[PaletteColor::TitleSecondary] = Color::Dark(BaseColor::Green);

    theme.palette = palette;
    siv.set_theme(theme);
}
