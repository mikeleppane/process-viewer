use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router, Server};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Write;
use std::sync::Arc;
use sysinfo::{Cpu, CpuExt, System, SystemExt};
use tokio::sync::Mutex;

const DEFAULT_PORT: u16 = 7070;

fn get_address() -> String {
    let port = env::var("PORT").unwrap_or(DEFAULT_PORT.to_string());
    format!("{}:{port}", "0.0.0.0")
}

fn router() -> Router {
    Router::new()
        .route("/api/cpus", get(get_cpus))
        .route("/health", get(health))
        .with_state(AppState {
            sys: Arc::new(Mutex::new(System::new())),
        })
}

#[tokio::main]
async fn main() {
    let server = Server::bind(&get_address().parse().expect("Invalid host given"))
        .serve(router().into_make_service());
    let addr = server.local_addr();
    println!("Listening on {addr}");
    server.await.expect("Failed while waiting for the server");
    println!("Hello, world!");
}

#[derive(Clone)]
struct AppState {
    sys: Arc<Mutex<System>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CpuInfo {
    cpu_usage: f32,
    frequency: u64,
    vendor_id: String,
    brand: String,
}

#[debug_handler]
async fn get_cpus(State(state): State<AppState>) -> impl IntoResponse {
    let mut sys = state.sys.lock().await;
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
    Json(cpus)
}

#[debug_handler]
async fn health() -> &'static str {
    "Ok"
}
