mod chat_client_tui;

use cursive::align::HAlign;
use cursive::theme::{BaseColor, Color, Palette, PaletteColor, Theme};
use cursive::traits::*;
use cursive::views::{Dialog, EditView, LinearLayout, TextView};

fn main() {
    let mut siv = cursive::default();
    set_custom_theme(&mut siv);

    show_mode_selection(&mut siv);

    siv.add_global_callback('q', |s| s.quit());

    siv.run();
}

fn show_mode_selection(siv: &mut cursive::Cursive) {
    // Create the options text
    let options =
        TextView::new("Enter 1 for Server Mode\nEnter 2 for Client Mode").h_align(HAlign::Center);

    // Create the input field
    let input = EditView::new()
        .on_submit(move |s, text| {
            match text.trim() {
                "1" => {
                    // Server mode
                    s.pop_layer();
                    s.add_layer(
                        Dialog::around(TextView::new("Server mode will be implemented next"))
                            .title("Coming Soon")
                            .button("Back", |s| {
                                s.pop_layer();
                                show_mode_selection(s);
                            })
                            .button("Quit", |s| s.quit()),
                    );
                }
                "2" => {
                    // Client mode - Call your chat client TUI
                    s.pop_layer();
                    s.quit(); // Quit the current Cursive instance

                    // Start the chat client TUI
                    chat_client_tui::run_chat_tui();
                }
                _ => {
                    // Invalid option
                    s.add_layer(Dialog::info("Please enter 1 or 2").title("Invalid Option"));
                }
            }
        })
        .with_name("input")
        .fixed_height(1);

    // Create the layout
    let layout = LinearLayout::vertical()
        .child(TextView::new("\n\n")) // Add some space at the top
        .child(options)
        .child(TextView::new("\n")) // Add space between options and input
        .child(input.full_width());

    // Add the layout to the screen
    siv.add_layer(Dialog::around(layout).title("Hotline Chat"));
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
