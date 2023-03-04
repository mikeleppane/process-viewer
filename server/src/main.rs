use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router, Server};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use sysinfo::{CpuExt, System, SystemExt};

const DEFAULT_PORT: u16 = 7070;

trait HumanReadable: Sized {
    fn to_human(self, precision: Option<u8>) -> String;
}

impl HumanReadable for u64 {
    fn to_human(self, precision: Option<u8>) -> String {
        let precision = if let Some(precision) = precision {
            precision
        } else {
            2
        };
        match self {
            0..=999 => self.to_string(),
            1000..=999_999 => {
                format!("{:.*} KB", precision as usize, self as f64 / 1000f64)
            }
            1_000_000..=999_999_999 => {
                format!("{:.*} MB", precision as usize, self as f64 / 1_000_000f64)
            }
            1_000_000_000.. => {
                format!(
                    "{:.*} GB",
                    precision as usize,
                    self as f64 / 1_000_000_000f64
                )
            }
        }
    }
}

fn get_address() -> String {
    let port = env::var("PORT").unwrap_or(DEFAULT_PORT.to_string());
    format!("{}:{port}", "0.0.0.0")
}

fn router(app_state: AppState) -> Router {
    Router::new()
        .route("/api/cpus", get(get_cpus))
        .route("/api/memory", get(get_memory))
        .route("/realtime/cpus", get(realtime_cpus_get))
        .route("/realtime/memory", get(realtime_memory_get))
        .route("/health", get(health))
        .with_state(app_state)
}

#[tokio::main]
async fn main() {
    let app_state = AppState::default();
    start_cpu_info_task(app_state.clone());
    start_memory_data_collection_task(app_state.clone());
    let server = Server::bind(&get_address().parse().expect("Invalid host given"))
        .serve(router(app_state).into_make_service());
    let addr = server.local_addr();
    println!("Listening on {addr}");
    server.await.expect("Failed while waiting for the server");
    println!("Hello, world!");
}

fn start_cpu_info_task(app_state: AppState) {
    tokio::task::spawn_blocking(move || {
        let mut sys = System::new();
        loop {
            sys.refresh_cpu();
            let cpus: Vec<CpuInfo> = sys
                .cpus()
                .iter()
                .map(|cpu| CpuInfo {
                    cpu_usage: cpu.cpu_usage(),
                    frequency: cpu.frequency(),
                    vendor_id: cpu.vendor_id().to_owned(),
                    brand: cpu.brand().to_owned(),
                })
                .collect();
            {
                let mut cpu_info = app_state.cpu_info.lock().unwrap();
                *cpu_info = cpus;
            }
            std::thread::sleep(System::MINIMUM_CPU_UPDATE_INTERVAL);
        }
    });
}

fn start_memory_data_collection_task(app_state: AppState) {
    tokio::task::spawn_blocking(move || {
        let mut sys = System::new();
        loop {
            sys.refresh_memory();
            let memory_data = Memory {
                total_memory: sys.total_memory().to_human(None),
                used_memory: sys.used_memory().to_human(None),
                total_swap: sys.total_swap().to_human(None),
                used_swap: sys.used_swap().to_human(None),
            };
            {
                let mut memory = app_state.memory.lock().unwrap();
                *memory = memory_data;
            }

            std::thread::sleep(System::MINIMUM_CPU_UPDATE_INTERVAL);
        }
    });
}

#[derive(Default, Clone)]
struct AppState {
    cpu_info: Arc<Mutex<Vec<CpuInfo>>>,
    memory: Arc<Mutex<Memory>>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct CpuInfo {
    cpu_usage: f32,
    frequency: u64,
    vendor_id: String,
    brand: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct Memory {
    total_memory: String,
    used_memory: String,
    total_swap: String,
    used_swap: String,
}

#[debug_handler]
async fn get_cpus(State(state): State<AppState>) -> impl IntoResponse {
    let cpu_info = state.cpu_info.lock().unwrap().clone();
    Json(cpu_info)
}

#[debug_handler]
async fn get_memory(State(state): State<AppState>) -> impl IntoResponse {
    let memory = state.memory.lock().unwrap().clone();
    Json(memory)
}

#[debug_handler]
async fn health() -> &'static str {
    "Ok"
}

#[debug_handler]
async fn realtime_cpus_get(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws| async { realtime_cpu_stream(state, ws).await })
}

async fn realtime_cpu_stream(app_state: AppState, mut ws: WebSocket) {
    loop {
        let payload = serde_json::to_string(&*app_state.cpu_info.lock().unwrap()).unwrap();
        ws.send(Message::Text(payload)).await.unwrap_or_default();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[debug_handler]
async fn realtime_memory_get(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws| async { crate::realtime_memory_stream(state, ws).await })
}

async fn realtime_memory_stream(app_state: AppState, mut ws: WebSocket) {
    loop {
        let payload = serde_json::to_string(&*app_state.memory.lock().unwrap()).unwrap();
        ws.send(Message::Text(payload)).await.unwrap_or_default();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
