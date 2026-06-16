use std::fs;
use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::models::{ProcessStats, SystemInfo};
use crate::AppError;

pub fn get_system_info() -> Result<SystemInfo, AppError> {
    let hostname = fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();

    let os = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("PRETTY_NAME=")))
        .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
        .unwrap_or_else(|| "Linux".to_string());

    let arch = std::env::consts::ARCH.to_string();

    let uptime_seconds = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;

    let meminfo = parse_proc_meminfo();
    let total_memory_mb = meminfo.get("MemTotal").copied().unwrap_or(0) / 1024;
    let available_memory_mb = meminfo.get("MemAvailable").copied().unwrap_or(0) / 1024;
    let used_memory_mb = total_memory_mb.saturating_sub(available_memory_mb);

    let cpu_usage_percent = read_cpu_usage();

    let (disk_total_mb, disk_used_mb) = read_disk_usage();

    Ok(SystemInfo {
        hostname,
        os,
        arch,
        uptime_seconds,
        total_memory_mb,
        used_memory_mb,
        cpu_usage_percent,
        disk_total_mb,
        disk_used_mb,
    })
}

pub fn get_process_stats(pid: u32) -> Result<ProcessStats, AppError> {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = fs::read_to_string(&stat_path)
        .map_err(|_| AppError::BadRequest(format!("Process {} not found", pid)))?;

    let parts = parse_stat(&stat_content);
    let utime: u64 = parts.get(13).and_then(|s| s.parse().ok()).unwrap_or(0);
    let stime: u64 = parts.get(14).and_then(|s| s.parse().ok()).unwrap_or(0);
    let rss_pages: i64 = parts.get(23).and_then(|s| s.parse().ok()).unwrap_or(0);

    let page_size = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) as u64 };
    let memory_bytes = if rss_pages > 0 {
        rss_pages as u64 * page_size
    } else {
        0
    };
    let memory_mb = memory_bytes / (1024 * 1024);

    let meminfo = parse_proc_meminfo();
    let total_mem_kb = meminfo.get("MemTotal").copied().unwrap_or(1) as f32;
    let memory_percent = if total_mem_kb > 0 {
        (memory_bytes / 1024) as f32 / total_mem_kb * 100.0
    } else {
        0.0
    };

    let (read_bytes, write_bytes) = read_proc_io(pid);

    // 简化 CPU 计算：仅返回基于进程时间的粗略估计
    let total_time = utime + stime;
    let cpu_percent = (total_time as f32 / 100.0).min(100.0); // 粗略估计

    Ok(ProcessStats {
        pid,
        cpu_percent,
        memory_mb,
        memory_percent,
        read_bytes,
        write_bytes,
    })
}

fn parse_proc_meminfo() -> std::collections::HashMap<String, u64> {
    let mut map = std::collections::HashMap::new();
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            let mut parts = line.split_whitespace();
            if let Some(key) = parts.next() {
                let key = key.trim_end_matches(':');
                if let Some(val) = parts.next() {
                    if let Ok(v) = val.parse::<u64>() {
                        map.insert(key.to_string(), v);
                    }
                }
            }
        }
    }
    map
}

static CPU_PREV: Lazy<Mutex<(u64, u64)>> = Lazy::new(|| Mutex::new((0, 0)));

fn read_cpu_usage() -> f32 {
    let content = match fs::read_to_string("/proc/stat") {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    let first_line = content.lines().next().unwrap_or("");
    let values: Vec<u64> = first_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();

    if values.len() < 4 {
        return 0.0;
    }

    let idle = values[3];
    let total: u64 = values.iter().take(7).sum();

    let mut prev = CPU_PREV.lock().unwrap();
    let total_diff = total.saturating_sub(prev.0);
    let idle_diff = idle.saturating_sub(prev.1);
    *prev = (total, idle);
    drop(prev);

    if total_diff == 0 {
        0.0
    } else {
        ((total_diff - idle_diff) as f32 / total_diff as f32) * 100.0
    }
}

fn read_disk_usage() -> (u64, u64) {
    #[cfg(target_os = "linux")]
    {
        let mut total = 0u64;
        let mut used = 0u64;
        if let Ok(content) = fs::read_to_string("/proc/mounts") {
            let mut seen = std::collections::HashSet::new();
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }
                let mount = parts[1];
                if mount.starts_with("/dev") || mount.starts_with("/run") || mount.starts_with("/sys") || mount.starts_with("/proc") || mount == "/boot" {
                    continue;
                }
                if !seen.insert(mount.to_string()) {
                    continue;
                }
                if let Ok(stat) = nix::sys::statvfs::statvfs(mount) {
                    let bsize = stat.block_size() as u64;
                    let blocks = stat.blocks() as u64;
                    let bavail = stat.blocks_available() as u64;
                    total += blocks * bsize / (1024 * 1024);
                    used += (blocks - bavail) * bsize / (1024 * 1024);
                }
            }
        }
        (total, used)
    }
    #[cfg(not(target_os = "linux"))]
    {
        (0, 0)
    }
}

fn read_proc_io(pid: u32) -> (u64, u64) {
    let mut read_bytes = 0u64;
    let mut write_bytes = 0u64;
    if let Ok(content) = fs::read_to_string(format!("/proc/{}/io", pid)) {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("read_bytes: ") {
                read_bytes = val.trim().parse().unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("write_bytes: ") {
                write_bytes = val.trim().parse().unwrap_or(0);
            }
        }
    }
    (read_bytes, write_bytes)
}

fn parse_stat(content: &str) -> Vec<String> {
    // 处理 command 中可能包含空格和括号的情况
    let mut result = Vec::new();
    let mut chars = content.chars().peekable();

    // pid
    let mut token = String::new();
    while let Some(&c) = chars.peek() {
        if c == ' ' {
            break;
        }
        token.push(c);
        chars.next();
    }
    result.push(token);
    while chars.peek() == Some(&' ') {
        chars.next();
    }

    // comm (inside parentheses)
    if chars.peek() == Some(&'(') {
        chars.next(); // skip '('
        let mut comm = String::new();
        let mut depth = 1;
        while let Some(c) = chars.next() {
            if c == '(' {
                depth += 1;
                comm.push(c);
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    break;
                } else {
                    comm.push(c);
                }
            } else {
                comm.push(c);
            }
        }
        result.push(comm);
    }
    while chars.peek() == Some(&' ') {
        chars.next();
    }

    // rest
    let rest: String = chars.collect();
    for s in rest.split_whitespace() {
        result.push(s.to_string());
    }

    result
}
