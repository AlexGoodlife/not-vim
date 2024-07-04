use simple_logging::log_to_file;
use std::env;
use std::io::stdout;

use crossterm::terminal;
use crate::client::Client;

pub mod client;
pub mod editor;
pub mod styles;



fn main() {
    let _args: Vec<String> = env::args().collect();
    log_to_file("editor.log", log::LevelFilter::Info).unwrap();

    let stdout = stdout();

    let dimensions = terminal::size().unwrap();
    log::info!("Started editor");
    log::info!("Dimensions: {} columns, {} rows", dimensions.0, dimensions.1);
    let mut client = Client::new(stdout, dimensions);
    client
        .editor
        .open_file("bible.txt")
        .map_err(|err| log::error!("Couldn't open file because of {err}"))
        .unwrap();
    let _ = client.run().map_err(|err| log::error!("{err}"));
}
