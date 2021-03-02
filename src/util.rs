use pi_hole_api::{OverTimeData, PiHoleAPI, Summary, TopClients, TopItems};
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{self};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

pub struct PiHoleData {
    pub summary: Option<Summary>,
    pub top_sources: Option<TopClients>,
    pub top_items: Option<TopItems>,
    pub over_time_data: Option<OverTimeData>,
}

struct BackgroundUpdater {
    handle: thread::JoinHandle<()>,
    receiver: mpsc::Receiver<Option<PiHoleData>>,
}

pub struct PiHoleServer {
    pub name: String,
    pub host: String,
    pub api_key: Option<String>,
    pub last_update: Instant,
    pub last_data: PiHoleData,
    background_updater: Option<BackgroundUpdater>,
}

impl PiHoleServer {
    pub fn run_background_update(&mut self) {
        if self.background_updater.is_none() {
            let (tx, rx) = mpsc::channel();
            let host = self.host.clone();
            let api_key = self.api_key.clone();
            let handle = thread::spawn(move || background_update(tx, host, api_key));

            self.background_updater = Some(BackgroundUpdater {
                handle,
                receiver: rx,
            });
        }
    }

    pub fn check_background_update(&mut self) {
        let mut join = false;
        match &self.background_updater {
            Some(background_updater) => match background_updater
                .receiver
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(option_pi_hole_data) => {
                    match option_pi_hole_data {
                        Some(pi_hole_data) => self.last_data = pi_hole_data,
                        None => {}
                    }
                    join = true;
                    self.last_update = Instant::now();
                }
                Err(_) => {}
            },
            None => {}
        }
        if join {
            self.background_updater = None;
        }
    }
}

pub struct App {
    pub selected_server_index: usize,
    pub servers: Vec<PiHoleServer>,
    pub update_delay: u64,
    pub graph_squash_factor: usize,
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
        server.check_background_update();
        if Instant::now().duration_since(server.last_update)
            > Duration::from_millis(self.update_delay)
        {
            server.run_background_update();
        }
    }

    pub fn on_space(&mut self) {
        let server = &mut self.servers[self.selected_server_index];
        server.run_background_update();
    }

    pub fn on_z(&mut self) {
        if self.graph_squash_factor > 1 {
            self.graph_squash_factor /= 2;
        }
    }

    pub fn on_x(&mut self) {
        if self.graph_squash_factor < usize::MAX {
            self.graph_squash_factor *= 2;
        }
    }

    pub fn on_e(&self) {
        let server = &self.servers[self.selected_server_index];
        let api = PiHoleAPI::new(server.host.clone(), server.api_key.clone());
        let mut rt = Runtime::new().expect("Failed to start async runtime");

        rt.block_on(async {
            api.enable().await.expect("Failed to enable pi-hole");
        });
    }

    pub fn on_d(&self) {
        let server = &self.servers[self.selected_server_index];
        let api = PiHoleAPI::new(server.host.clone(), server.api_key.clone());
        let mut rt = Runtime::new().expect("Failed to start async runtime");

        rt.block_on(async {
            api.disable(60).await.expect("Failed to disable pi-hole");
        });
    }
}

impl From<PimonConfig> for App {
    fn from(config: PimonConfig) -> Self {
        App {
            selected_server_index: 0,
            update_delay: config.update_delay,
            graph_squash_factor: 1,
            servers: config
                .servers
                .iter()
                .map(|server| PiHoleServer {
                    name: server.name.clone(),
                    host: server.host.clone(),
                    api_key: server.api_key.clone(),
                    last_update: Instant::now()
                        .checked_sub(Duration::from_millis(config.update_delay))
                        .expect("Failed to set last update"),
                    last_data: PiHoleData {
                        summary: None,
                        top_sources: None,
                        top_items: None,
                        over_time_data: None,
                    },
                    background_updater: None,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PimonConfig {
    servers: Vec<PiHoleServerConfig>,
    update_delay: u64,
}

#[derive(Debug, Deserialize)]
struct PiHoleServerConfig {
    name: String,
    host: String,
    api_key: Option<String>,
}

pub fn load_server_from_json(path: &PathBuf) -> Result<App, Box<dyn Error>> {
    let f = File::open(path).expect("Configuration file not found");
    let pimon_config: PimonConfig = serde_json::from_reader(&f)?;
    Ok(App::from(pimon_config))
}

pub fn order_convert_string_num_map(map: &HashMap<String, u64>) -> Vec<Vec<String>> {
    let mut selected_items: Vec<(String, &u64)> = map
        .iter()
        .map(|(domain, count)| (domain.clone(), count))
        .collect();
    selected_items.sort_by(|a, b| (b.1, &b.0).cmp(&(a.1, &a.0)));
    selected_items
        .iter()
        .map(|(domain, count)| vec![domain.clone(), count.to_string()])
        .collect()
}

fn background_update(tx: mpsc::Sender<Option<PiHoleData>>, host: String, api_key: Option<String>) {
    let api = PiHoleAPI::new(host.clone(), api_key);
    let mut rt = Runtime::new().expect("Failed to start async runtime");

    rt.block_on(async {
        tx.send(Some(PiHoleData {
            summary: api.get_summary().await.ok(),
            top_sources: api.get_top_clients(None).await.ok(),
            top_items: api.get_top_items(None).await.ok(),
            over_time_data: api.get_over_time_data_10_mins().await.ok(),
        }))
    })
    .unwrap();
}

pub fn squash_queries_over_time(
    queries: &Vec<(&i64, &u64)>,
    squash_factor: usize,
) -> Vec<(i64, u64)> {
    let mut squashed = Vec::new();
    let mut count = 0;
    let mut sum = 0;
    let mut leading_timestamp = 0;

    for (timestamp, query_count) in queries {
        if count == 0 {
            leading_timestamp = **timestamp;
        }
        count += 1;
        sum += **query_count;
        if count >= squash_factor {
            squashed.push((leading_timestamp, sum));
            count = 0;
            sum = 0;
        }
    }
    if count > 0 {
        squashed.push((leading_timestamp, sum));
    }

    squashed
}
