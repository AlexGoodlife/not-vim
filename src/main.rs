use simple_logging::log_to_file;
use std::fs::OpenOptions;
use std::io::stdout;
use std::io::Write;
use std::{env, panic};

use crate::client::Client;
use crossterm::terminal;

pub mod client;
pub mod editor;
pub mod styles;
pub mod ui;

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() < 2 { "test.txt" } else { &args[1] };
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
    log::info!(
        "Dimensions: {} columns, {} rows",
        dimensions.0,
        dimensions.1
    );
    let _ = client
        .editor
        .open_file(file_path)
        .map_err(|err| println!("Couldn't open file{err}"));
    let _ = client.run().map_err(|err| log::error!("{err}"));
}
