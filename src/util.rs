use pi_hole_api::{
    api_types::{OverTimeData, Summary, TopClients, TopItems},
    AuthenticatedPiHoleAPI, PiHoleAPIConfig, PiHoleAPIConfigWithKey, UnauthenticatedPiHoleAPI,
};
use serde::Deserialize;
use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{self};
use std::thread;
use std::time::{Duration, Instant};

pub struct PiHoleData {
    pub summary: Option<Summary>,
    pub top_sources: Option<TopClients>,
    pub top_items: Option<TopItems>,
    pub over_time_data: Option<OverTimeData>,
}

pub enum PiHoleConfigImplementation {
    Default(PiHoleAPIConfig),
    WithKey(PiHoleAPIConfigWithKey),
}

impl PiHoleConfigImplementation {
    pub fn new(host: String, api_key: Option<String>) -> Self {
        match api_key {
            Some(key) => {
                PiHoleConfigImplementation::WithKey(PiHoleAPIConfigWithKey::new(host, key))
            }
            None => PiHoleConfigImplementation::Default(PiHoleAPIConfig::new(host)),
        }
    }

    pub fn get_unauthenticated_api(&self) -> Option<&dyn UnauthenticatedPiHoleAPI> {
        Some(match self {
            Self::Default(config) => config,
            Self::WithKey(config) => config,
        })
    }

    pub fn get_authenticated_api(&self) -> Option<&dyn AuthenticatedPiHoleAPI> {
        match self {
            Self::Default(_) => None,
            Self::WithKey(config) => Some(config),
        }
    }
}

struct BackgroundUpdater {
    handle: thread::JoinHandle<()>,
    receiver: mpsc::Receiver<Option<PiHoleData>>,
}

pub struct PiHoleServer {
    pub name: String,
    pub host: String,
    pub api_key: Option<String>,
    pub api_config: PiHoleConfigImplementation,
    pub last_update: Instant,
    pub last_data: PiHoleData,
    background_updater: Option<BackgroundUpdater>,
}

impl PiHoleServer {
    pub fn new(
        name: String,
        host: String,
        api_key: Option<String>,
        update_delay: Duration,
    ) -> Self {
        let api_config = PiHoleConfigImplementation::new(host.clone(), api_key.clone());
        PiHoleServer {
            name: name,
            host: host,
            api_key: api_key,
            api_config: api_config,
            last_update: Instant::now()
                .checked_sub(update_delay)
                .expect("Failed to set last update"),
            last_data: PiHoleData {
                summary: None,
                top_sources: None,
                top_items: None,
                over_time_data: None,
            },
            background_updater: None,
        }
    }
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
            if let Some(background_updater) = self.background_updater.take() {
                background_updater
                    .handle
                    .join()
                    .expect("Unable to join background updater thread");
            }
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

    pub fn on_e(&mut self) {
        let server = &mut self.servers[self.selected_server_index];
        match server.api_config.get_authenticated_api() {
            None => {}
            Some(api) => {
                api.enable().expect("Failed to enable pi-hole");
            }
        };
        server.run_background_update();
    }

    pub fn on_d(&mut self) {
        let server = &mut self.servers[self.selected_server_index];
        match server.api_config.get_authenticated_api() {
            None => {}
            Some(api) => {
                api.disable(60).expect("Failed to disable pi-hole");
            }
        };
        server.run_background_update();
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
                .map(|server| {
                    PiHoleServer::new(
                        server.name.clone(),
                        server.host.clone(),
                        server.api_key.clone(),
                        Duration::from_millis(config.update_delay),
                    )
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
    let api_config = PiHoleConfigImplementation::new(host, api_key);

    tx.send(Some(PiHoleData {
        summary: api_config
            .get_unauthenticated_api()
            .and_then(|api| api.get_summary().ok()),
        top_sources: api_config
            .get_authenticated_api()
            .and_then(|api| api.get_top_clients(Some(25)).ok()),
        top_items: api_config
            .get_authenticated_api()
            .and_then(|api| api.get_top_items(Some(25)).ok()),
        over_time_data: api_config
            .get_unauthenticated_api()
            .and_then(|api| api.get_over_time_data_10_mins().ok()),
    }))
    .unwrap();
}

pub fn squash_queries_over_time(
    queries: &Vec<(i64, u64)>,
    squash_factor: usize,
) -> Vec<(i64, u64)> {
    let mut squashed = Vec::new();
    let mut count = 0;
    let mut sum = 0;
    let mut leading_timestamp = 0;

    for (timestamp, query_count) in queries {
        if count == 0 {
            leading_timestamp = *timestamp;
        }
        count += 1;
        sum += query_count;
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
