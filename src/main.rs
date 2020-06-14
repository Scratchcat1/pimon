mod event;
mod ui;
mod util;

use event::{Config, Event, Events};
use std::path::PathBuf;
use std::{error::Error, io, time::Duration};
use structopt::StructOpt;
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{backend::TermionBackend, Terminal};

#[derive(StructOpt)]
struct Cli {
    /// Path to configuration file
    #[structopt(short, long, default_value("pimon.json"))]
    config_file_path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Cli::from_args();

    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Setup event handlers
    let events = Events::with_config(Config {
        tick_rate: Duration::from_millis(2000),
        ..Config::default()
    });

    let mut app = util::load_server_from_json(&args.config_file_path)?;

    app.on_tick();

    loop {
        terminal.draw(|mut f| ui::draw_ui(&mut f, &mut app))?;

        match events.next()? {
            Event::Input(key) => match key {
                Key::Char('q') => {
                    break;
                }
                Key::Left => {
                    app.previous_server();
                }
                Key::Right => {
                    app.next_server();
                }
                Key::Char(' ') => {
                    app.on_space();
                }
                Key::Char('z') => {
                    app.on_z();
                }
                Key::Char('x') => {
                    app.on_x();
                }
                _ => {}
            },
            Event::Tick => {
                app.on_tick();
            }
        }
    }

    Ok(())
}
