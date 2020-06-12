mod event;
use event::{Config, Event, Events};
use pi_hole_api::{OverTimeData, PiHoleAPI, Summary, TopClients, TopItems};
use std::collections::HashMap;
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tokio::runtime::Runtime;
use tui::{
    backend::Backend,
    backend::TermionBackend,
    layout::{Constraint, Corner, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{BarChart, Block, BorderType, Borders, List, Paragraph, Row, Table, Tabs, Text},
    Frame, Terminal,
};

pub struct PiHoleData {
    pub summary: Option<Summary>,
    pub top_sources: Option<TopClients>,
    pub top_items: Option<TopItems>,
    pub over_time_data: Option<OverTimeData>,
}

pub struct PiHoleServer {
    pub name: String,
    pub host: String,
    pub api_key: Option<String>,
    pub last_update: Instant,
    pub last_data: PiHoleData,
}

pub struct App {
    pub selected_server_index: usize,
    pub servers: Vec<PiHoleServer>,
    pub update_delay: Duration,
}

impl App {
    pub fn next_server(&mut self) {
        self.selected_server_index = (self.selected_server_index + 1) % self.servers.len();
    }

    pub fn previous_server(&mut self) {
        if self.selected_server_index > 0 {
            self.selected_server_index -= 1;
        } else {
            self.selected_server_index = self.servers.len() - 1;
        }
    }

    pub fn on_tick(&mut self) {
        let server = &mut self.servers[self.selected_server_index];
        if Instant::now().duration_since(server.last_update) > self.update_delay {
            let api = PiHoleAPI::new(server.host.clone(), server.api_key.clone());
            let mut rt = Runtime::new().expect("Failed to start async runtime");

            rt.block_on(async {
                server.last_data.summary = api.get_summary().await.ok();
                server.last_data.top_sources = api.get_top_clients(None).await.ok();
                server.last_data.top_items = api.get_top_items(None).await.ok();
                server.last_data.over_time_data = api.get_over_time_data_10_mins().await.ok();
            })
        }
    }
}

fn draw_tabs<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let server_names: Vec<&String> = app.servers.iter().map(|server| &server.name).collect();
    let tabs = Tabs::default()
        .block(Block::default().borders(Borders::ALL).title("Pi Hole"))
        .titles(&server_names)
        .style(Style::default().fg(Color::Green))
        .highlight_style(Style::default().fg(Color::Yellow))
        .select(app.selected_server_index);
    f.render_widget(tabs, area);
}

fn draw_overview<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ]
            .as_ref(),
        )
        .split(area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Overview");

    match &app.servers[app.selected_server_index].last_data.summary {
        Some(summary) => {
            let text = vec![
                Text::raw("Status: "),
                Text::styled(
                    format!("{}\n", summary.status),
                    Style::default().fg(Color::Red),
                ),
                Text::raw("Version: Unknown\n"),
            ];
            let paragraph = Paragraph::new(text.iter()).block(block);
            f.render_widget(paragraph, chunks[0])
        }
        None => f.render_widget(block, chunks[0]),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Overview")
        .title_style(Style::default().fg(Color::Yellow));
    f.render_widget(block, chunks[1]);
}

fn draw_queries_chart<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let block = Block::default()
        .title("Total queries")
        .borders(Borders::ALL);
    match &app.servers[app.selected_server_index]
        .last_data
        .over_time_data
    {
        Some(over_time_data) => {
            let queries_over_time_rows: Vec<(String, u64)> = over_time_data
                .domains_over_time
                .iter()
                .map(|(timestamp, count)| (timestamp.to_string(), *count))
                .collect();

            let queries_over_time_str_rows: Vec<(&str, u64)> = queries_over_time_rows
                .iter()
                .map(|(timestamp, count)| (timestamp.as_str(), *count))
                .collect();
            let barchart = BarChart::default()
                .block(block)
                .data(&queries_over_time_str_rows)
                .bar_width(4)
                .style(Style::default().fg(Color::Yellow))
                .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));
            f.render_widget(barchart, area);
        }
        None => f.render_widget(block, area),
    };
}

fn draw_list<B>(
    f: &mut Frame<B>,
    app: &mut App,
    area: Rect,
    title: String,
    header: &Vec<String>,
    rows: &Vec<Vec<String>>,
) where
    B: Backend,
{
    let up_style = Style::default().fg(Color::Green);
    let rows = rows.iter().map(|row| {
        let style = up_style;
        Row::StyledData(row.iter(), style)
    });
    let table = Table::new(header.iter(), rows)
        .block(Block::default().title(&title).borders(Borders::ALL))
        .header_style(Style::default().fg(Color::Yellow))
        .widths(&[Constraint::Percentage(70), Constraint::Percentage(30)]);
    f.render_widget(table, area);
}

fn order_convert_string_num_map(map: &HashMap<String, u64>) -> Vec<Vec<String>> {
    let mut selected_items: Vec<(String, &u64)> = map
        .iter()
        .map(|(domain, count)| (domain.clone(), count))
        .collect();
    selected_items.sort_by(|a, b| b.1.cmp(&a.1));
    selected_items
        .iter()
        .map(|(domain, count)| vec![domain.clone(), count.to_string()])
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
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

    let mut app = App {
        selected_server_index: 0,
        servers: vec![PiHoleServer {
            name: "Test server".to_string(),
            host: "http://192.168.0.100".to_string(),
            api_key: Some(
                "".to_string(),
            ),
            last_update: Instant::now(),
            last_data: PiHoleData {
                summary: None,
                top_items: None,
                top_sources: None,
                over_time_data: None,
            },
        }],
        update_delay: Duration::from_millis(10000),
    };

    loop {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(8),
                        Constraint::Percentage(12),
                        Constraint::Percentage(40),
                        Constraint::Percentage(40),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            // Pi Hole tabs
            draw_tabs(&mut f, &mut app, chunks[0]);

            // Overview
            draw_overview(&mut f, &mut app, chunks[1]);

            // Queries chart
            // Vec<String, u64>
            draw_queries_chart(&mut f, &mut app, chunks[2]);

            // Top domains
            {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(
                        [
                            Constraint::Ratio(1, 3),
                            Constraint::Ratio(1, 3),
                            Constraint::Ratio(1, 3),
                        ]
                        .as_ref(),
                    )
                    .split(chunks[3]);

                let top_queries_rows =
                    match &app.servers[app.selected_server_index].last_data.top_items {
                        Some(top_items) => order_convert_string_num_map(&top_items.top_queries),
                        None => Vec::new(),
                    };

                let top_ads_rows = match &app.servers[app.selected_server_index].last_data.top_items
                {
                    Some(top_items) => order_convert_string_num_map(&top_items.top_ads),
                    None => Vec::new(),
                };

                let top_clients_rows =
                    match &app.servers[app.selected_server_index].last_data.top_sources {
                        Some(top_sources) => order_convert_string_num_map(&top_sources.top_sources),
                        None => Vec::new(),
                    };

                let header = vec!["Domain".to_string(), "Count".to_string()];
                draw_list(
                    &mut f,
                    &mut app,
                    chunks[0],
                    "Top Queries".to_string(),
                    &header,
                    &top_queries_rows,
                );

                let header = vec!["Domain".to_string(), "Count".to_string()];
                draw_list(
                    &mut f,
                    &mut app,
                    chunks[1],
                    "Top Ads".to_string(),
                    &header,
                    &top_ads_rows,
                );

                let header = vec!["Client".to_string(), "Count".to_string()];
                draw_list(
                    &mut f,
                    &mut app,
                    chunks[2],
                    "Top Clients".to_string(),
                    &header,
                    &top_clients_rows,
                );
            }
        })?;

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
                _ => {}
            },
            Event::Tick => {
                app.on_tick();
            }
        }
    }

    Ok(())
}
