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
use tokio::sync::broadcast;
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
    let (tx_cpu, _) = broadcast::channel::<Vec<CpuInfo>>(1);
    let (tx_memory, _) = broadcast::channel::<Memory>(1);
    let app_state = AppState {
        tx_cpu: tx_cpu.clone(),
        tx_memory: tx_memory.clone(),
        cpu_info: Arc::new(Mutex::new(vec![])),
        memory: Arc::new(Mutex::new(Memory::default())),
    };
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
            app_state.tx_cpu.send(cpus).unwrap_or_default();
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
            app_state.tx_memory.send(memory_data).unwrap_or_default();
            std::thread::sleep(System::MINIMUM_CPU_UPDATE_INTERVAL);
        }
    });
}

#[derive(Clone)]
struct AppState {
    tx_cpu: broadcast::Sender<Vec<CpuInfo>>,
    tx_memory: broadcast::Sender<Memory>,
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
    let mut rx = app_state.tx_cpu.subscribe();
    while let Ok(msg) = rx.recv().await {
        let payload = serde_json::to_string(&msg).unwrap();
        ws.send(Message::Text(payload)).await.unwrap_or_default();
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
    let mut rx = app_state.tx_memory.subscribe();
    while let Ok(msg) = rx.recv().await {
        let payload = serde_json::to_string(&msg).unwrap();
        ws.send(Message::Text(payload)).await.unwrap_or_default();
    }
}
