use simple_logging::log_to_file;
use std::fs::OpenOptions;
use std::{env, panic};
use std::io::stdout;
use std::io::Write;

use crossterm::terminal;
use crate::client::Client;

pub mod client;
pub mod editor;
pub mod styles;



fn main() {
    let _args: Vec<String> = env::args().collect();
    log_to_file("editor.log", log::LevelFilter::Info).unwrap();

    let stdout = stdout();
    panic::set_hook(Box::new(|panic_info| {
        // Open the file in append mode
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("editor.log")
            .expect("Could not open panic log file");

        // Write the panic info to the file
        writeln!(file, "{}", panic_info).expect("Could not write to panic log file");
    }));

    // This will trigger a panic
    let dimensions = terminal::size().unwrap();
    let mut client = Client::new(stdout, dimensions);
    log::info!("Started editor");
    log::info!("Dimensions: {} columns, {} rows", dimensions.0, dimensions.1);
    client
        .editor
        .open_file("test.txt")
        .map_err(|err| log::error!("Couldn't open file because of {err}"))
        .unwrap();
    let _ = client.run().map_err(|err| log::error!("{err}"));
}
