use std::path::PathBuf;

/// 应用在用户目录下的子文件夹名（APPNAME）。
const APP_DIR: &str = "chronospace-reader";

/// 用户级数据目录：`<用户配置根>/local/<APPNAME>`。
///
/// - Linux (Mint)：`~/.config/local/chronospace-reader`
/// - macOS：`~/Library/Application Support/local/chronospace-reader`
/// - Windows：`%APPDATA%\local\chronospace-reader`
///
/// 可通过环境变量 `CHRONOSPACE_DATA_DIR` 覆盖（便于测试或自定义部署）。
pub fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("CHRONOSPACE_DATA_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    config_root().join("local").join(APP_DIR)
}

pub fn db_path() -> PathBuf {
    data_dir().join("chronospace.sqlite3")
}

pub fn llm_config_path() -> PathBuf {
    data_dir().join("llm_config.json")
}

/// 确保数据目录存在并返回之。
pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(target_os = "macos")]
fn config_root() -> PathBuf {
    home().join("Library").join("Application Support")
}

#[cfg(target_os = "windows")]
fn config_root() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home().join("AppData").join("Roaming"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn config_root() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg);
        }
    }
    home().join(".config")
}

fn home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_respects_env_override() {
        let key = "CHRONOSPACE_DATA_DIR";
        let previous = std::env::var(key).ok();
        std::env::set_var(key, "/tmp/chronospace-test-override");

        assert_eq!(data_dir(), PathBuf::from("/tmp/chronospace-test-override"));
        assert_eq!(db_path(), PathBuf::from("/tmp/chronospace-test-override/chronospace.sqlite3"));
        assert_eq!(
            llm_config_path(),
            PathBuf::from("/tmp/chronospace-test-override/llm_config.json")
        );

        match previous {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn data_dir_contains_app_namespace() {
        // 无论是否设置覆盖，路径都应包含应用命名空间关键字。
        let s = data_dir().to_string_lossy().to_string();
        assert!(s.contains("chronospace"), "实际路径: {s}");
    }
}
