use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use chrono::Utc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::models::{Program, ProgramStatus};
use crate::protect::{prevent_self_stop, validate_work_dir};
use crate::AppError;

pub struct ProcessManager {
    data_dir: PathBuf,
    programs: HashMap<String, Program>,
    running: HashMap<String, RunningProcess>,
    save_path: PathBuf,
}

struct RunningProcess {
    stop_tx: mpsc::Sender<()>,
}

impl ProcessManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let save_path = data_dir.join("programs.json");
        Self {
            data_dir,
            programs: HashMap::new(),
            running: HashMap::new(),
            save_path,
        }
    }

    pub async fn load_programs(&mut self) -> anyhow::Result<()> {
        if self.save_path.exists() {
            let content = fs::read_to_string(&self.save_path).await?;
            let programs: Vec<Program> = serde_json::from_str(&content)?;
            for p in programs {
                self.programs.insert(p.id.clone(), p);
            }
        }
        Ok(())
    }

    async fn save_programs(&self) -> anyhow::Result<()> {
        let programs: Vec<&Program> = self.programs.values().collect();
        let content = serde_json::to_string_pretty(&programs)?;
        let mut file = fs::File::create(&self.save_path).await?;
        file.write_all(content.as_bytes()).await?;
        Ok(())
    }

    pub async fn auto_start(&mut self) {
        let ids: Vec<String> = self
            .programs
            .values()
            .filter(|p| p.auto_start)
            .map(|p| p.id.clone())
            .collect();
        for id in ids {
            if let Err(e) = self.start(&id).await {
                warn!("Auto-start failed for {}: {}", id, e);
            }
        }
    }

    pub fn list(&self) -> Vec<&Program> {
        self.programs.values().collect()
    }

    pub fn get(&self, id: &str) -> Option<&Program> {
        self.programs.get(id)
    }

    pub async fn create(&mut self, mut program: Program) -> Result<String, AppError> {
        validate_work_dir(&program.work_dir, &self.data_dir)?;
        tokio::fs::create_dir_all(&program.work_dir)
            .await
            .map_err(|e| AppError::BadRequest(format!("Invalid work directory: {}", e)))?;
        let id = program.id.clone();
        self.programs.insert(id.clone(), program);
        self.save_programs().await.map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(id)
    }

    pub async fn update(&mut self, id: &str, updates: crate::models::ProgramUpdateRequest) -> Result<(), AppError> {
        let program = self.programs.get_mut(id).ok_or(AppError::NotFound)?;
        if program.status == ProgramStatus::Running {
            return Err(AppError::BadRequest(
                "Program is running, please stop it before updating configuration".to_string(),
            ));
        }
        if let Some(name) = updates.name {
            program.name = name;
        }
        if let Some(command) = updates.command {
            program.command = command;
        }
        if let Some(work_dir) = updates.work_dir {
            validate_work_dir(&work_dir, &self.data_dir)?;
            program.work_dir = work_dir;
        }
        if let Some(args) = updates.args {
            program.args = args;
        }
        if let Some(env) = updates.env {
            program.env = env;
        }
        if let Some(auto_start) = updates.auto_start {
            program.auto_start = auto_start;
        }
        self.save_programs().await.map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    pub async fn delete(&mut self, id: &str) -> Result<(), AppError> {
        let program = self.programs.get(id).ok_or(AppError::NotFound)?;
        if program.status == ProgramStatus::Running {
            return Err(AppError::BadRequest(
                "Program is running, please stop it before deleting".to_string(),
            ));
        }
        self.programs.remove(id);
        self.save_programs().await.map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    pub async fn start(&mut self, id: &str) -> Result<(), AppError> {
        let program = self.programs.get(id).ok_or(AppError::NotFound)?.clone();
        if program.status == ProgramStatus::Running {
            return Err(AppError::BadRequest("Program is already running".to_string()));
        }

        let log_dir = self.data_dir.join("logs").join(&program.id);
        fs::create_dir_all(&log_dir).await.map_err(|e| AppError::Internal(e.to_string()))?;
        let stdout_path = log_dir.join("stdout.log");
        let stderr_path = log_dir.join("stderr.log");

        let stdout_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stdout_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to open stdout log: {}", e)))?;
        let stderr_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_path)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to open stderr log: {}", e)))?;

        let mut cmd = Command::new(&program.command);
        cmd.current_dir(&program.work_dir)
            .args(&program.args)
            .envs(&program.env)
            .stdout(Stdio::from(stdout_file.into_std().await))
            .stderr(Stdio::from(stderr_file.into_std().await))
            .kill_on_drop(false);

        let mut child = cmd.spawn().map_err(|e| {
            AppError::Internal(format!("Failed to spawn process: {}", e))
        })?;

        let pid = child.id();
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

        let id_clone = id.to_string();
        tokio::spawn(async move {
            tokio::select! {
                _ = child.wait() => {}
                _ = stop_rx.recv() => {
                    if let Some(pid) = pid {
                        let _ = nix::sys::signal::kill(
                            nix::unistd::Pid::from_raw(pid as i32),
                            nix::sys::signal::Signal::SIGTERM,
                        );
                    }
                }
            }
            // Ensure process is gone
            if let Some(pid) = pid {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
            info!("Program {} exited", id_clone);
        });

        self.running.insert(id.to_string(), RunningProcess { stop_tx });

        if let Some(p) = self.programs.get_mut(id) {
            p.status = ProgramStatus::Running;
            p.pid = pid;
            p.started_at = Some(Utc::now());
        }
        self.save_programs().await.map_err(|e| AppError::Internal(e.to_string()))?;
        info!("Started program {} with PID {:?}", id, pid);
        Ok(())
    }

    pub async fn stop(&mut self, id: &str) -> Result<(), AppError> {
        let program = self.programs.get(id).ok_or(AppError::NotFound)?;
        if program.status != ProgramStatus::Running {
            return Err(AppError::BadRequest("Program is not running".to_string()));
        }
        if let Some(pid) = program.pid {
            prevent_self_stop(pid)?;
        }

        if let Some(running) = self.running.remove(id) {
            let _ = running.stop_tx.send(()).await;
        }

        if let Some(p) = self.programs.get_mut(id) {
            p.status = ProgramStatus::Stopped;
            p.pid = None;
            p.started_at = None;
        }
        self.save_programs().await.map_err(|e| AppError::Internal(e.to_string()))?;
        info!("Stopped program {}", id);
        Ok(())
    }

    pub async fn restart(&mut self, id: &str) -> Result<(), AppError> {
        let _ = self.stop(id).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(id).await
    }

    pub async fn refresh_status(&mut self) {
        let ids: Vec<String> = self.programs.keys().cloned().collect();
        for id in ids {
            if let Some(program) = self.programs.get_mut(&id) {
                if program.status == ProgramStatus::Running {
                    if let Some(pid) = program.pid {
                        if !is_process_alive(pid) {
                            program.status = ProgramStatus::Error;
                            program.pid = None;
                            program.started_at = None;
                            self.running.remove(&id);
                        }
                    } else {
                        program.status = ProgramStatus::Error;
                        self.running.remove(&id);
                    }
                }
            }
        }
    }

    pub fn log_dir(&self, id: &str) -> PathBuf {
        self.data_dir.join("logs").join(id)
    }
}

fn is_process_alive(pid: u32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok()
}
