use crate::util::{self, App};
use chrono::{DateTime, NaiveDateTime, Utc};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{BarChart, Block, BorderType, Borders, Paragraph, Row, Table, Tabs, Text},
    Frame,
};

pub fn draw_tabs<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let server_names: Vec<&String> = app.servers.iter().map(|server| &server.name).collect();
    let tabs = Tabs::default()
        .block(Block::default().borders(Borders::ALL).title("Pi Hole"))
        .titles(&server_names)
        .style(Style::default().fg(Color::Yellow))
        .highlight_style(Style::default().fg(Color::Green))
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
                    "enabled" => Color::Green,
                    _ => Color::Red,
                };
                let text = vec![
                    Text::raw("Status: "),
                    Text::styled(
                        format!("{}\n", summary.status),
                        Style::default().fg(styled_status_colour),
                    ),
                    Text::raw(format!(
                        "API key: {}\n",
                        !&app.servers[app.selected_server_index].api_key.is_none()
                    )),
                    Text::raw(format!("Privacy level: {}\n", &summary.privacy_level)),
                    Text::raw(format!(
                        "Blocklist size: {}\n",
                        &summary.domains_being_blocked
                    )),
                ];
                let paragraph = Paragraph::new(text.iter()).block(summary_block);
                f.render_widget(paragraph, chunks[0]);
            }
            {
                let text = vec![
                    Text::raw(format!("Queries: {}\n", &summary.dns_queries_today)),
                    Text::raw(format!("Ads blocked: {}\n", &summary.ads_blocked_today)),
                    Text::raw(format!("Ads percent: {}\n", &summary.ads_percentage_today)),
                    Text::raw(format!("Unique domains: {}\n", &summary.unique_domains)),
                ];
                let paragraph = Paragraph::new(text.iter()).block(query_stats_block);
                f.render_widget(paragraph, chunks[1]);
            }

            {
                let text = vec![
                    Text::raw(format!("Forwarded: {}\n", &summary.queries_forwarded)),
                    Text::raw(format!("Cached: {}\n", &summary.queries_cached)),
                    Text::raw(format!("Unique clients: {}\n", &summary.unique_clients)),
                    // Text::raw(format!("Gravity update: {} days\n", &summary.unique_clients)),
                ];
                let paragraph = Paragraph::new(text.iter()).block(other_stats_block);
                f.render_widget(paragraph, chunks[2]);
            }

            {
                let text = vec![
                    Text::raw(format!("NODATA: {}\n", &summary.reply_nodata)),
                    Text::raw(format!("NXDOMAIN: {}\n", &summary.reply_nxdomain)),
                    Text::raw(format!("CNAME: {}\n", &summary.reply_cname)),
                    Text::raw(format!("IP: {}\n", &summary.reply_ip)),
                    // Text::raw(format!("Gravity update: {} days\n", &summary.unique_clients)),
                ];
                let paragraph = Paragraph::new(text.iter()).block(responses_block);
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
            let mut queries_over_time_rows: Vec<(&i64, &u64)> =
                over_time_data.domains_over_time.iter().collect();

            // Display with left as the latest entry.
            // Otherwise the data is cut off on the right side.
            queries_over_time_rows.sort_by(|a, b| b.0.cmp(a.0));
            let queries_over_time_rows: Vec<(String, u64)> = queries_over_time_rows
                .iter()
                .map(|(timestamp, count)| {
                    let naive = NaiveDateTime::from_timestamp(**timestamp, 0);
                    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
                    (datetime.format("%H:%M").to_string(), **count)
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
                .style(Style::default().fg(Color::Green))
                .value_style(Style::default().fg(Color::Black).bg(Color::Green));
            f.render_widget(bar_chart, area);
        }
        None => f.render_widget(block, area),
    };
}

pub fn draw_list<B>(
    f: &mut Frame<B>,
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
        .header_style(Style::default().fg(Color::LightCyan))
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
                Constraint::Length(3),
                Constraint::Length(6),
                Constraint::Percentage(40),
                Constraint::Percentage(40),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Pi Hole tabs
    draw_tabs(f, app, chunks[0]);

    // Overview
    draw_overview(f, app, chunks[1]);

    // Queries chart
    // Vec<String, u64>
    draw_queries_chart(f, app, chunks[2]);

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
        draw_list(
            f,
            chunks[0],
            "Top Queries".to_string(),
            &header,
            &top_queries_rows,
        );

        let header = vec!["Domain".to_string(), "Count".to_string()];
        draw_list(f, chunks[1], "Top Ads".to_string(), &header, &top_ads_rows);

        let header = vec!["Client".to_string(), "Count".to_string()];
        draw_list(
            f,
            chunks[2],
            "Top Clients".to_string(),
            &header,
            &top_clients_rows,
        );
    }
}
