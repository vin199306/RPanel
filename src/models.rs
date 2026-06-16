use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::process_mgr::ProcessManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub id: String,
    pub name: String,
    pub command: String,
    pub work_dir: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub auto_start: bool,
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    pub status: ProgramStatus,
    #[serde(skip)]
    pub pid: Option<u32>,
    #[serde(skip)]
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProgramStatus {
    Running,
    Stopped,
    Error,
}

impl Default for ProgramStatus {
    fn default() -> Self {
        ProgramStatus::Stopped
    }
}

impl Program {
    pub fn new(name: String, command: String, work_dir: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            command,
            work_dir,
            args: Vec::new(),
            env: HashMap::new(),
            auto_start: false,
            created_at: Utc::now(),
            status: ProgramStatus::Stopped,
            pid: None,
            started_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramRequest {
    pub name: String,
    pub command: String,
    pub work_dir: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub auto_start: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramUpdateRequest {
    pub name: Option<String>,
    pub command: Option<String>,
    pub work_dir: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub auto_start: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgramResponse {
    pub id: String,
    pub name: String,
    pub command: String,
    pub work_dir: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub auto_start: bool,
    pub created_at: DateTime<Utc>,
    pub status: String,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<i64>,
}

impl From<&Program> for ProgramResponse {
    fn from(p: &Program) -> Self {
        let uptime_seconds = p.started_at.map(|s| (Utc::now() - s).num_seconds());
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            command: p.command.clone(),
            work_dir: p.work_dir.clone(),
            args: p.args.clone(),
            env: p.env.clone(),
            auto_start: p.auto_start,
            created_at: p.created_at,
            status: match p.status {
                ProgramStatus::Running => "running".to_string(),
                ProgramStatus::Stopped => "stopped".to_string(),
                ProgramStatus::Error => "error".to_string(),
            },
            pid: p.pid,
            uptime_seconds,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub uptime_seconds: u64,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub cpu_usage_percent: f32,
    pub disk_total_mb: u64,
    pub disk_used_mb: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessStats {
    pub pid: u32,
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub memory_percent: f32,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub content: String,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub config_path: PathBuf,
    pub config: Arc<RwLock<crate::config::AppConfig>>,
    pub process_manager: Arc<RwLock<ProcessManager>>,
}
