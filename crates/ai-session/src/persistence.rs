use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::session::Session;

/// セッションの一覧表示用サマリ
pub struct SessionSummary {
    pub id: Uuid,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub message_count: usize,
    pub expired: bool,
}

/// セッションの保存・読み込み・削除を管理する
pub struct SessionManager {
    storage_dir: PathBuf,
}

impl SessionManager {
    pub fn new(storage_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(storage_dir)
            .with_context(|| format!("failed to create {}", storage_dir.display()))?;
        Ok(Self {
            storage_dir: storage_dir.to_owned(),
        })
    }

    /// 保存済みセッションの一覧を返す（新しい順）
    pub fn list(&self) -> Result<Vec<SessionSummary>> {
        let mut summaries = Vec::new();

        let entries = std::fs::read_dir(&self.storage_dir)
            .with_context(|| format!("failed to read {}", self.storage_dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(session) = self.load_from_path(&path) {
                    let expired = session.is_expired();
                    summaries.push(SessionSummary {
                        id: session.id,
                        title: session.title,
                        created_at: session.created_at,
                        last_active: session.last_active,
                        message_count: session.messages.len(),
                        expired,
                    });
                }
            }
        }

        summaries.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        Ok(summaries)
    }

    /// セッションを読み込む
    pub fn load(&self, id: &Uuid) -> Result<Session> {
        let path = self.session_path(id);
        self.load_from_path(&path)
    }

    /// セッションを保存する
    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.id);
        let json = serde_json::to_string_pretty(session)
            .context("failed to serialize session")?;
        std::fs::write(&path, json)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    /// セッションを削除する
    pub fn delete(&self, id: &Uuid) -> Result<()> {
        let path = self.session_path(id);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to delete {}", path.display()))?;
        }
        Ok(())
    }

    /// 期限切れセッションを削除し、削除数を返す
    pub fn cleanup_expired(&self) -> Result<usize> {
        let sessions = self.list()?;
        let mut count = 0;
        for summary in &sessions {
            if summary.expired {
                self.delete(&summary.id)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// プレフィックスからセッション ID を解決する（短縮 UUID 対応）
    pub fn resolve_prefix(&self, prefix: &str) -> Result<Uuid> {
        let sessions = self.list()?;
        let matches: Vec<_> = sessions
            .iter()
            .filter(|s| s.id.to_string().starts_with(prefix))
            .collect();

        match matches.len() {
            0 => anyhow::bail!("no session matching prefix '{prefix}'"),
            1 => Ok(matches[0].id),
            n => anyhow::bail!("ambiguous prefix '{prefix}': {n} sessions match"),
        }
    }

    fn session_path(&self, id: &Uuid) -> PathBuf {
        self.storage_dir.join(format!("{id}.json"))
    }

    fn load_from_path(&self, path: &Path) -> Result<Session> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let session: Session = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(dir.path()).unwrap();

        let session = Session::new(Some("test".to_string()), Some(3600));
        manager.save(&session).unwrap();

        let loaded = manager.load(&session.id).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.system_prompt, Some("test".to_string()));
    }

    #[test]
    fn list_returns_sessions_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(dir.path()).unwrap();

        let s1 = Session::new(None, None);
        let mut s2 = Session::new(None, None);
        s2.last_active = s1.last_active + chrono::Duration::seconds(10);

        manager.save(&s1).unwrap();
        manager.save(&s2).unwrap();

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, s2.id); // newer first
    }

    #[test]
    fn delete_removes_session() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(dir.path()).unwrap();

        let session = Session::new(None, None);
        manager.save(&session).unwrap();
        manager.delete(&session.id).unwrap();

        assert!(manager.load(&session.id).is_err());
    }

    #[test]
    fn cleanup_removes_expired() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(dir.path()).unwrap();

        let active = Session::new(None, Some(3600));
        let mut expired = Session::new(None, Some(1));
        expired.last_active = Utc::now() - chrono::Duration::seconds(100);

        manager.save(&active).unwrap();
        manager.save(&expired).unwrap();

        let count = manager.cleanup_expired().unwrap();
        assert_eq!(count, 1);
        assert_eq!(manager.list().unwrap().len(), 1);
    }
}
