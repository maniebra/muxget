use std::path::PathBuf;

use muxget::controllers::app::App;

fn main() -> std::io::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // -d DIR, else $PWD.
    let dir = match args.iter().position(|a| a == "-d") {
        Some(i) if i + 1 < args.len() => PathBuf::from(args.drain(i..i + 2).nth(1).unwrap()),
        _ => std::env::current_dir()?,
    };

    let mut app = App::new(dir);
    for url in &args {
        app.add(url);
    }

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
