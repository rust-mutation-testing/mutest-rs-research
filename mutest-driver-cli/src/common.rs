use clap::builder::{Styles};

pub static DEFAULT_JSON_DIR: &str = "./mutest-data";

pub fn clap_styles() -> Styles {
    use clap::builder::styling::*;
    Styles::styled()
        .header(Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightGreen))).bold())
        .usage(Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightGreen))).bold())
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlue))).bold())
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::BrightBlue))))
}