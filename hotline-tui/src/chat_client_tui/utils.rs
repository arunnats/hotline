use super::imports::*;

pub fn print_textline_to_output(siv_sink: &CbSink, content: &TextContent, textline: TextLine) {
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

pub fn print_chat_message_to_output(
    siv_sink: &CbSink,
    content: &TextContent,
    message: ChatMessage,
) {
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
