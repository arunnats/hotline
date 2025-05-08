use super::imports::*;

// MOVED TO /shared
// pub fn global_quit(siv_sink: &CbSink, shutdown_signal: &Arc<AtomicBool>) {
//     // Set the shutdown signal
//     shutdown_signal.store(true, Ordering::SeqCst);

//     // Send a quit command to the UI
//     let sink = siv_sink.clone();
//     sink.send(Box::new(move |s| {
//         s.quit();
//     }))
//     .unwrap_or(());

//     // Spawn a thread that will force exit if clean shutdown takes too long
//     std::thread::spawn(move || {
//         // Give a shorter time for clean shutdown
//         std::thread::sleep(Duration::from_millis(400));
//         // Force exit the process
//         std::process::exit(0);
//     });
// }

pub fn print_textline_to_output(
    siv_sink: &CbSink,
    content: &TextContent,
    textline: TextLine,
    auto_scroll: &Arc<Mutex<bool>>,
) {
    let content = content.clone();
    let sink = siv_sink.clone();
    let auto_scroll = auto_scroll.clone();

    sink.send(Box::new(move |s| {
        let mut styled = StyledString::new();
        if let Some(serializable_color) = textline.color {
            // Convert SerializableColor to cursive::theme::Color
            let color: Color = serializable_color.into();
            styled.append_styled(format!("{}\n", textline.text), color);
        } else {
            styled.append_styled(format!("{}\n", textline.text), Color::TerminalDefault);
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
    message: ChatMessage,
    auto_scroll: &Arc<Mutex<bool>>,
) {
    let content = content.clone();
    let sink = siv_sink.clone();
    let auto_scroll = auto_scroll.clone();

    sink.send(Box::new(move |s| {
        let mut styled = StyledString::new();

        // Format timestamp
        let local_time = message.timestamp.with_timezone(&Local).format("%H:%M:%S");
        styled.append_styled(
            format!("[{}] ", local_time),
            Color::Light(BaseColor::Yellow),
        );

        // Format sender name
        let sender_name = message.username.unwrap_or(message.sender);
        let display_name = if message.is_self { "You" } else { &sender_name };

        styled.append_styled(
            format!("{}: ", display_name),
            Color::Light(BaseColor::Green),
        );

        // Format content
        styled.append_styled(
            format!("{}\n", message.content),
            Color::Light(BaseColor::Cyan),
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
