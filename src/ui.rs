use crate::util::{self, App};
use chrono::{DateTime, NaiveDateTime, Utc};
use std::str::FromStr;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Span, Spans, Text},
    widgets::{BarChart, Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

pub fn draw_help_bar<B>(f: &mut Frame<B>, area: Rect)
where
    B: Backend,
{
    let text = Text::raw(
        "E: Enable  D: Disable  Z: Zoom+  X: Zoom-  Space: Update  LArrow: Prev  RArrow: Next",
    );
    let paragraph = Paragraph::new(text).style(Style::default().bg(Color::Cyan));
    f.render_widget(paragraph, area);
}

pub fn draw_tabs<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let server_names = app
        .servers
        .iter()
        .map(|server| &server.name)
        .cloned()
        .map(|server_name| {
            Spans::from(vec![Span::styled(
                server_name,
                Style::default().fg(Color::LightYellow),
            )])
        })
        .collect();
    let tabs = Tabs::new(server_names)
        .block(Block::default().borders(Borders::ALL).title("Pi Hole"))
        .highlight_style(Style::default().fg(Color::LightGreen))
        .select(app.selected_server_index);
    f.render_widget(tabs, area);
}

pub fn draw_overview<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
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
    let summary_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Summary");

    let query_stats_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Query stats");

    let other_stats_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Other stats");

    let responses_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title("Responses");

    match &app.servers[app.selected_server_index].last_data.summary {
        Some(summary) => {
            {
                let styled_status_colour = match summary.status.as_str() {
                    "enabled" => Color::LightGreen,
                    _ => Color::Red,
                };
                let styled_api_key_colour = match &app.servers[app.selected_server_index].api_key {
                    Some(_) => Color::LightGreen,
                    None => Color::Red,
                };

                let text = vec![
                    Spans::from(vec![
                        Span::raw("Status: "),
                        Span::styled(
                            format!("{}", summary.status),
                            Style::default().fg(styled_status_colour),
                        ),
                    ]),
                    Spans::from(vec![
                        Span::raw("API key: "),
                        Span::styled(
                            format!(
                                "{}",
                                !&app.servers[app.selected_server_index].api_key.is_none()
                            ),
                            Style::default().fg(styled_api_key_colour),
                        ),
                    ]),
                    Spans::from(vec![Span::raw(format!(
                        "Privacy level: {}",
                        &summary.privacy_level
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "Blocklist size: {}",
                        &summary.domains_being_blocked
                    ))]),
                ];
                let paragraph = Paragraph::new(text).block(summary_block);
                f.render_widget(paragraph, chunks[0]);
            }
            {
                let text = vec![
                    Spans::from(vec![Span::raw(format!(
                        "Queries: {}",
                        &summary.dns_queries_today
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "Ads blocked: {}",
                        &summary.ads_blocked_today
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "Ads percent: {}",
                        &summary.ads_percentage_today
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "Unique domains: {}",
                        &summary.unique_domains
                    ))]),
                ];
                let paragraph = Paragraph::new(text).block(query_stats_block);
                f.render_widget(paragraph, chunks[1]);
            }

            {
                let text = vec![
                    Spans::from(vec![Span::raw(format!(
                        "Forwarded: {}",
                        &summary.queries_forwarded
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "Cached: {}",
                        &summary.queries_cached
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "Unique clients: {}",
                        &summary.unique_clients
                    ))]),
                ];
                let paragraph = Paragraph::new(text).block(other_stats_block);
                f.render_widget(paragraph, chunks[2]);
            }

            {
                let text = vec![
                    Spans::from(vec![Span::raw(format!(
                        "NODATA: {}",
                        &summary.reply_nodata
                    ))]),
                    Spans::from(vec![Span::raw(format!(
                        "NXDOMAIN: {}",
                        &summary.reply_nxdomain
                    ))]),
                    Spans::from(vec![Span::raw(format!("CNAME: {}", &summary.reply_cname))]),
                    Spans::from(vec![Span::raw(format!("IP: {}", &summary.reply_ip))]),
                ];
                let paragraph = Paragraph::new(text).block(responses_block);
                f.render_widget(paragraph, chunks[3]);
            }
        }
        None => {
            f.render_widget(summary_block, chunks[0]);
            f.render_widget(query_stats_block, chunks[1]);
            f.render_widget(other_stats_block, chunks[2]);
            f.render_widget(responses_block, chunks[3]);
        }
    };
}

pub fn draw_queries_chart<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
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
            let mut queries_over_time_rows: Vec<(i64, u64)> = over_time_data
                .domains_over_time
                .iter()
                .map(|(time, count)| (i64::from_str(time).unwrap(), *count))
                .collect();

            // Display with left as the latest entry.
            // Otherwise the data is cut off on the right side.
            queries_over_time_rows.sort_by(|a, b| b.0.cmp(&a.0));
            let squashed_queries_over_time =
                util::squash_queries_over_time(&queries_over_time_rows, app.graph_squash_factor);
            let queries_over_time_rows: Vec<(String, u64)> = squashed_queries_over_time
                .iter()
                .map(|(timestamp, count)| {
                    let naive = NaiveDateTime::from_timestamp(*timestamp, 0);
                    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
                    (datetime.format("%H:%M").to_string(), *count)
                })
                .collect();

            let queries_over_time_str_rows: Vec<(&str, u64)> = queries_over_time_rows
                .iter()
                .map(|(timestamp, count)| (timestamp.as_str(), *count))
                .collect();
            let bar_chart = BarChart::default()
                .block(block)
                .data(&queries_over_time_str_rows)
                .bar_width(5)
                .bar_style(Style::default().fg(Color::Green))
                .value_style(Style::default().fg(Color::Black).bg(Color::Green));
            f.render_widget(bar_chart, area);
        }
        None => f.render_widget(block, area),
    };
}

pub fn draw_statistics<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
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
        .split(area);

    let top_queries_rows = match &app.servers[app.selected_server_index].last_data.top_items {
        Some(top_items) => util::order_convert_string_num_map(&top_items.top_queries),
        None => Vec::new(),
    };

    let top_ads_rows = match &app.servers[app.selected_server_index].last_data.top_items {
        Some(top_items) => util::order_convert_string_num_map(&top_items.top_ads),
        None => Vec::new(),
    };

    let top_clients_rows = match &app.servers[app.selected_server_index].last_data.top_sources {
        Some(top_sources) => util::order_convert_string_num_map(&top_sources.top_sources),
        None => Vec::new(),
    };

    let header = vec!["Domain".to_string(), "Count".to_string()];
    draw_list(f, chunks[0], "Top Queries", &header, &top_queries_rows);

    let header = vec!["Domain".to_string(), "Count".to_string()];
    draw_list(f, chunks[1], "Top Ads", &header, &top_ads_rows);

    let header = vec!["Client".to_string(), "Count".to_string()];
    draw_list(f, chunks[2], "Top Clients", &header, &top_clients_rows);
}

pub fn draw_list<B>(
    f: &mut Frame<B>,
    area: Rect,
    title: &str,
    header: &Vec<String>,
    rows: &Vec<Vec<String>>,
) where
    B: Backend,
{
    let up_style = Style::default().fg(Color::LightGreen);
    let rows = rows.iter().map(|row| {
        let style = up_style;
        Row::new(row.iter().map(|text| Cell::from(text.clone()).style(style)))
    });
    let table = Table::new(rows)
        .block(
            Block::default()
                .title(vec![Span::from(title)])
                .borders(Borders::ALL),
        )
        .header(
            Row::new(header.iter().map(|text| Cell::from(text.clone())))
                .style(Style::default().fg(Color::LightCyan)),
        )
        .widths(&[Constraint::Percentage(70), Constraint::Percentage(30)]);
    f.render_widget(table, area);
}

pub fn draw_ui<B>(f: &mut Frame<B>, app: &mut App)
where
    B: Backend,
{
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(6),
                Constraint::Percentage(40),
                Constraint::Percentage(40),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Help bar
    draw_help_bar(f, chunks[0]);

    // Pi Hole tabs
    draw_tabs(f, app, chunks[1]);

    // Overview
    draw_overview(f, app, chunks[2]);

    // Queries chart
    draw_queries_chart(f, app, chunks[3]);

    // Top domains
    draw_statistics(f, app, chunks[4]);
}
