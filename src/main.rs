use crossterm::Result;

pub mod editor;


fn main() -> Result<()> {
    let mut ed = editor::Editor::new();
    ed.run().expect("Something crashed");
    Ok(())
}

