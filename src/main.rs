// mod custom_event;
mod ui;
mod util;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
// use custom_event::{Config, CustomEvent, CustomEvents};
use std::path::PathBuf;
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use structopt::StructOpt;
use tui::{backend::CrosstermBackend, Terminal};

#[derive(StructOpt)]
struct Cli {
    /// Path to configuration file
    #[structopt(short, long, default_value("pimon.json"))]
    config_file_path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Cli::from_args();

    let mut app = util::load_server_from_json(&args.config_file_path)?;

    if app.servers.len() == 0 {
        println!("Configuration file doesn't contain any servers. Exiting");
        std::process::exit(1);
    }

    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // let stdout = MouseTerminal::from(stdout);
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    // terminal.hide_cursor()?;

    // // Setup event handlers
    // let events = Events::with_config(Config {
    //     tick_rate: Duration::from_millis(1000),
    //     ..Config::default()
    // });

    app.on_tick();
    let tick_rate = Duration::from_millis(1000);
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|mut f| ui::draw_ui(&mut f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Left => {
                        app.previous_server();
                    }
                    KeyCode::Right => {
                        app.next_server();
                    }
                    KeyCode::Char(' ') => {
                        app.on_space();
                    }
                    KeyCode::Char('z') => {
                        app.on_z();
                    }
                    KeyCode::Char('x') => {
                        app.on_x();
                    }
                    KeyCode::Char('e') => {
                        app.on_e();
                    }
                    KeyCode::Char('d') => {
                        app.on_d();
                    }
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
