mod client;
mod server;
mod shared;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    // clap struct to get the inputs along with the run command
    #[arg(short, long, default_value = "client")] // defaullts to client
    mode: String,
}

// basic implementation, need to change it to have the three core functionalities and then internal choices
// prolly should make a splash screen + menu
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.mode.as_str() {
        "server" => server::run()?,
        "client" => client::run()?,
        _ => anyhow::bail!("Invalid mode. Use 'server' or 'client'"),
    }

    Ok(())
}
