use std::path::Path;

use crate::AppError;

/// 阻止用户停止面板自身进程
pub fn prevent_self_stop(pid: u32) -> Result<(), AppError> {
    let self_pid = std::process::id();
    if pid == self_pid {
        return Err(AppError::Forbidden(
            "Cannot stop or restart the panel itself".to_string(),
        ));
    }
    Ok(())
}

/// 检查工作目录是否为受保护路径
pub fn validate_work_dir(work_dir: &str, data_dir: &std::path::Path) -> Result<(), AppError> {
    let path = std::path::Path::new(work_dir);
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // 如果目录不存在，仅做简单字符串检查
            let wd = work_dir.trim_end_matches('/');
            let dd = data_dir.to_string_lossy().trim_end_matches('/').to_string();
            if wd == dd || wd.starts_with(&format!("{}/", dd)) {
                return Err(AppError::Forbidden(
                    "Work directory cannot be the panel data directory or its subdirectory".to_string(),
                ));
            }
            for blocked in BLOCKED_PATHS {
                let b = blocked.trim_end_matches('/');
                if wd == b || wd.starts_with(&format!("{}/", b)) {
                    return Err(AppError::Forbidden(format!(
                        "Work directory cannot be system critical directory: {}",
                        blocked
                    )));
                }
            }
            return Ok(());
        }
    };

    let data_canonical = match data_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => data_dir.to_path_buf(),
    };

    if canonical == data_canonical || canonical.starts_with(&data_canonical) {
        return Err(AppError::Forbidden(
            "Work directory cannot be the panel data directory or its subdirectory".to_string(),
        ));
    }

    for blocked in BLOCKED_PATHS {
        let blocked_path = Path::new(blocked);
        if let Ok(blocked_canonical) = blocked_path.canonicalize() {
            if canonical == blocked_canonical || canonical.starts_with(&blocked_canonical) {
                return Err(AppError::Forbidden(format!(
                    "Work directory cannot be system critical directory: {}",
                    blocked
                )));
            }
        } else if canonical.starts_with(blocked_path) {
            return Err(AppError::Forbidden(format!(
                "Work directory cannot be system critical directory: {}",
                blocked
            )));
        }
    }

    Ok(())
}

const BLOCKED_PATHS: &[&str] = &[
    "/",
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/lib",
    "/lib64",
    "/usr/lib",
    "/usr/lib64",
    "/etc",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/run",
    "/var/run",
];
