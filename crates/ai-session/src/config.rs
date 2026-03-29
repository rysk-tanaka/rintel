use std::path::PathBuf;
use std::time::Duration;

/// セッション設定
pub struct SessionConfig {
    /// セッションの有効期限（None = 無期限）
    pub default_ttl: Option<Duration>,
    /// セッション保存ディレクトリ
    pub storage_dir: PathBuf,
}

impl Default for SessionConfig {
    fn default() -> Self {
        // CLI ツールとして XDG 規約に従い ~/.config/ を使用
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        let storage_dir = base.join("rintel").join("sessions");
        Self {
            default_ttl: Some(Duration::from_secs(60 * 60)), // 1 hour
            storage_dir,
        }
    }
}
