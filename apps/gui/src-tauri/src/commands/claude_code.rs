use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde::Serialize;

#[derive(Serialize)]
pub struct ClaudeProject {
    pub dir_name: String,
    pub decoded_path: String,
}

#[derive(Serialize)]
pub struct ClaudeSessionSummary {
    pub session_id: String,
    pub slug: Option<String>,
    pub timestamp: Option<String>,
    pub message_count: usize,
}

#[derive(Serialize)]
pub struct ClaudeToolUse {
    pub name: String,
    pub input_preview: String,
}

#[derive(Serialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub timestamp: Option<String>,
    pub text_content: String,
    pub tool_uses: Vec<ClaudeToolUse>,
    pub uuid: Option<String>,
}

#[derive(Serialize)]
pub struct ClaudeSessionDetail {
    pub session_id: String,
    pub slug: Option<String>,
    pub git_branch: Option<String>,
    pub messages: Vec<ClaudeMessage>,
}

fn claude_projects_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let dir = home.join(".claude").join("projects");
    if !dir.is_dir() {
        return Err(format!("{} does not exist", dir.display()));
    }
    Ok(dir)
}

/// Validate that the resolved path is still under the base directory (prevent symlink escape).
fn validate_under_base(path: &PathBuf, base: &PathBuf) -> Result<(), String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve path: {e}"))?;
    let canonical_base = base
        .canonicalize()
        .map_err(|e| format!("cannot resolve base: {e}"))?;
    if !canonical.starts_with(&canonical_base) {
        return Err("path escapes base directory".to_string());
    }
    Ok(())
}

/// Validate that the input is a single normal path component (no separators or traversal).
fn validate_path_component(s: &str) -> Result<(), String> {
    if s.is_empty()
        || s.contains('/')
        || s.contains('\\')
        || s.contains("..")
        || s.starts_with('.')
    {
        return Err("invalid path component".to_string());
    }
    Ok(())
}

/// Decode Claude Code's encoded directory name back to a real filesystem path.
/// The encoding replaces `/` with `-`, but actual path segments may contain hyphens.
/// We greedily match against the real filesystem to find the correct split points.
fn decode_project_path(dir_name: &str) -> String {
    let raw = dir_name.strip_prefix('-').unwrap_or(dir_name);
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.is_empty() {
        return dir_name.replace('-', "/");
    }

    let mut resolved = PathBuf::from("/");
    let mut i = 0;
    while i < parts.len() {
        // Try longest possible segment first (greedy)
        let mut matched = false;
        for end in (i + 1..=parts.len()).rev() {
            let candidate = parts[i..end].join("-");
            let test_path = resolved.join(&candidate);
            if test_path.exists() {
                resolved = test_path;
                i = end;
                matched = true;
                break;
            }
        }
        if !matched {
            // Fallback: treat single part as a segment
            resolved.push(parts[i]);
            i += 1;
        }
    }

    resolved.to_string_lossy().into_owned()
}

#[tauri::command]
pub fn list_claude_projects() -> Result<Vec<ClaudeProject>, String> {
    let dir = match claude_projects_dir() {
        Ok(d) => d,
        Err(_) => return Ok(Vec::new()),
    };
    let mut projects: Vec<ClaudeProject> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            // Skip "memory" and other non-project directories
            if !dir_name.starts_with('-') {
                return None;
            }
            let decoded_path = decode_project_path(&dir_name);
            Some(ClaudeProject {
                dir_name,
                decoded_path,
            })
        })
        .collect();
    projects.sort_by(|a, b| a.decoded_path.cmp(&b.decoded_path));
    Ok(projects)
}

#[tauri::command]
pub fn list_claude_sessions(project_dir: String) -> Result<Vec<ClaudeSessionSummary>, String> {
    validate_path_component(&project_dir)?;
    let base = claude_projects_dir()?;
    let dir = base.join(&project_dir);
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    validate_under_base(&dir, &base)?;

    let mut sessions: Vec<ClaudeSessionSummary> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let session_id = name.strip_suffix(".jsonl")?.to_owned();
            if uuid::Uuid::parse_str(&session_id).is_err() {
                return None;
            }
            Some(extract_session_summary(&dir.join(&name), session_id))
        })
        .collect();

    sessions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(sessions)
}

fn extract_session_summary(path: &PathBuf, session_id: String) -> ClaudeSessionSummary {
    let mut slug = None;
    let mut timestamp = None;
    let mut message_count: usize = 0;

    if let Ok(file) = fs::File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };

            let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let is_compact = val
                .get("isCompactSummary")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let is_sidechain = val
                .get("isSidechain")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if (msg_type == "user" || msg_type == "assistant") && !is_compact && !is_sidechain {
                message_count += 1;
            }

            if slug.is_none() {
                if let Some(s) = val.get("slug").and_then(|v| v.as_str()) {
                    slug = Some(s.to_owned());
                }
            }
            if timestamp.is_none() {
                if let Some(t) = val.get("timestamp").and_then(|v| v.as_str()) {
                    timestamp = Some(t.to_owned());
                }
            }
        }
    }

    ClaudeSessionSummary {
        session_id,
        slug,
        timestamp,
        message_count,
    }
}

#[tauri::command]
pub fn get_claude_session(
    project_dir: String,
    session_id: String,
) -> Result<ClaudeSessionDetail, String> {
    validate_path_component(&project_dir)?;
    if uuid::Uuid::parse_str(&session_id).is_err() {
        return Err("invalid session id".to_string());
    }
    let base = claude_projects_dir()?;
    let path = base.join(&project_dir).join(format!("{session_id}.jsonl"));

    if !path.is_file() {
        return Err(format!("session file not found: {}", path.display()));
    }
    validate_under_base(&path, &base)?;

    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);

    let mut messages = Vec::new();
    let mut slug = None;
    let mut git_branch = None;

    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if msg_type != "user" && msg_type != "assistant" {
            continue;
        }

        let is_compact = val
            .get("isCompactSummary")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let is_sidechain = val
            .get("isSidechain")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_compact || is_sidechain {
            continue;
        }

        // Extract metadata from first matching line
        if slug.is_none() {
            slug = val
                .get("slug")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
        }
        if git_branch.is_none() {
            git_branch = val
                .get("gitBranch")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned());
        }

        let timestamp = val
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let uuid = val
            .get("uuid")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let (text_content, tool_uses) = extract_message_content(&val, msg_type);

        messages.push(ClaudeMessage {
            role: msg_type.to_owned(),
            timestamp,
            text_content,
            tool_uses,
            uuid,
        });
    }

    Ok(ClaudeSessionDetail {
        session_id,
        slug,
        git_branch,
        messages,
    })
}

fn extract_message_content(val: &serde_json::Value, msg_type: &str) -> (String, Vec<ClaudeToolUse>) {
    let mut text_content = String::new();
    let mut tool_uses = Vec::new();

    let Some(message) = val.get("message") else {
        return (text_content, tool_uses);
    };

    let Some(content) = message.get("content") else {
        return (text_content, tool_uses);
    };

    if msg_type == "user" {
        // User messages: content is a string
        if let Some(s) = content.as_str() {
            text_content = s.to_owned();
        } else if let Some(arr) = content.as_array() {
            // Sometimes user content is also an array
            for block in arr {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    if !text_content.is_empty() {
                        text_content.push('\n');
                    }
                    text_content.push_str(text);
                }
            }
        }
    } else if msg_type == "assistant" {
        // Assistant messages: content is an array of blocks
        if let Some(arr) = content.as_array() {
            for block in arr {
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                            if !text_content.is_empty() {
                                text_content.push('\n');
                            }
                            text_content.push_str(text);
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_owned();
                        let input_preview = block
                            .get("input")
                            .map(|v| {
                                let s = v.to_string();
                                if s.len() > 200 {
                                    let truncated: String = s.chars().take(200).collect();
                                    format!("{truncated}...")
                                } else {
                                    s
                                }
                            })
                            .unwrap_or_default();
                        tool_uses.push(ClaudeToolUse {
                            name,
                            input_preview,
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    (text_content, tool_uses)
}
