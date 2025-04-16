use chrono::{DateTime, Local, Utc};
use core::*;
use cursive::CbSink;
use cursive::align::HAlign;
use cursive::theme::{BaseColor, Color, Palette, PaletteColor, Theme};
use cursive::traits::*;
use cursive::utils::markup::StyledString;
use cursive::views::{Dialog, EditView, LinearLayout, TextContent, TextView};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;

use crate::client_backend::*;
use crate::serializable_colours::*;
use crate::types::{ChatMessage, OutputEvent, SerializableColor, SystemEvent, TextLine};

fn spawn_output_handler(
    mut output_rx: Receiver<OutputEvent>,
    siv_sink: CbSink,
    content: TextContent,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            while let Some(event) = output_rx.recv().await {
                match event {
                    OutputEvent::TextLine(line) => {
                        print_textline_to_output(&siv_sink, &content, line);
                    }
                    OutputEvent::ChatMessage(msg) => {
                        print_chat_message_to_output(&siv_sink, &content, msg);
                    }
                    OutputEvent::SystemEvent(event) => {
                        handle_system_event(&siv_sink, &content, event);
                    }
                }
            }
        });
    });
}

fn print_textline_to_output(siv_sink: &CbSink, content: &TextContent, textline: TextLine) {
    let content = content.clone();
    let sink = siv_sink.clone();
    sink.send(Box::new(move |_| {
        let mut styled = StyledString::new();
        if let Some(serializable_color) = textline.color {
            // Convert SerializableColor to cursive::theme::Color
            let color: Color = serializable_color.into();
            styled.append_styled(format!("{}\n", textline.text), color);
        } else {
            styled.append_styled(format!("{}\n", textline.text), Color::TerminalDefault);
        }
        content.append(styled);
    }))
    .unwrap();
}

fn print_chat_message_to_output(siv_sink: &CbSink, content: &TextContent, message: ChatMessage) {
    let content = content.clone();
    let sink = siv_sink.clone();

    sink.send(Box::new(move |_| {
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
    }))
    .unwrap();
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

#[tokio::main]
async fn main() {
    // Create channels for communication between UI and backend
    let (ui_to_backend_tx, ui_to_backend_rx) = mpsc::channel::<String>(100);
    let (backend_to_ui_tx, backend_to_ui_rx) = mpsc::channel::<OutputEvent>(100);

    // Start the backend in a separate thread
    let backend_handle = tokio::spawn(async move {
        if let Err(e) = run_client_backend(ui_to_backend_rx, backend_to_ui_tx.clone()).await {
            eprintln!("Backend error: {}", e);
        }
    });

    // Run the UI with the channels
    run_tui(ui_to_backend_tx, backend_to_ui_rx);
}

fn run_tui(input_tx: mpsc::Sender<String>, output_rx: mpsc::Receiver<OutputEvent>) {
    let mut siv = cursive::default();
    set_custom_theme(&mut siv);

    let content = TextContent::new("");
    let content_clone = content.clone();
    let siv_sink = siv.cb_sink().clone();

    // Add welcome messages from the frontend
    // Create SerializableColor for blue
    let blue_color = SerializableColor { r: 0, g: 0, b: 255 };

    print_textline_to_output(
        &siv_sink,
        &content,
        TextLine {
            text: "Welcome to Hotline Chat!".to_string(),
            color: Some(blue_color),
        },
    );

    // Create SerializableColor for white
    let white_color = SerializableColor {
        r: 255,
        g: 255,
        b: 255,
    };

    print_textline_to_output(
        &siv_sink,
        &content,
        TextLine {
            text: "Please follow the prompts to connect to a server.".to_string(),
            color: Some(white_color),
        },
    );

    let messages = TextView::new_with_content(content.clone())
        .scrollable()
        .full_height()
        .fixed_height(20);

    let input_label = TextView::new("Enter message (type '/quit' to exit)").h_align(HAlign::Left);

    let input_tx_clone = input_tx.clone();
    let input = EditView::new()
        .on_submit(move |s, text| {
            if !text.trim().is_empty() {
                let rt = Runtime::new().unwrap();
                rt.block_on(input_tx_clone.send(text.to_string())).unwrap();

                s.call_on_name("input", |view: &mut EditView| {
                    view.set_content("");
                });
            }
        })
        .with_name("input")
        .fixed_height(1);

    let layout = LinearLayout::vertical()
        .child(messages)
        .child(input_label)
        .child(input.full_width());

    siv.add_layer(Dialog::around(layout).title("Hotline Chat"));

    siv.add_global_callback('q', |s| s.quit());

    spawn_output_handler(output_rx, siv.cb_sink().clone(), content_clone);

    siv.run();
}

/// Sets a custom theme for the TUI
fn set_custom_theme(siv: &mut cursive::CursiveRunnable) {
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
