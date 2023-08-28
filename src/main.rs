use std::fs;
use std::env;

pub mod editor;
pub mod client;
mod util;

fn main(){
    let args : Vec<String> = env::args().collect();
    
    // println!("{:?}",args);
    // let buff = fs::read_to_string(args[1].to_string()).unwrap();
    // println!("{:?}", buff);
    let mut ed = client::Client::new();
    ed.open_file(&args[1]);
    let _ = ed.run().map_err( |err| {eprintln!("{:?}", err)});
}
