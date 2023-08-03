
pub mod editor;
mod client;


fn main(){
    let mut ed = client::Client::new();
    let _ = ed.run().map_err( |err| {eprintln!("{:?}", err)});
}

