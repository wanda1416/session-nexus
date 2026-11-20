use std::fs;
use std::path::Path;

use crate::models::{Ownership, Session, SessionStatus};
use crate::provider::{expand_user_profile, ProviderProfile};

const MAX_SESSIONS: usize = 200;

/// Index native session directories.
///
/// Grok and DSH store sessions two levels down (`<project>/<session-id>`).
/// A flat directory is indexed at the first level. This does not parse transcript files.
pub fn discover_sessions(profile: &ProviderProfile) -> Vec<Session> {
    let root = expand_user_profile(&profile.session_dir);
    let path = Path::new(&root);
    if !path.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    let Ok(top) = fs::read_dir(path) else {
        return found;
    };

    for entry in top.flatten() {
        if found.len() >= MAX_SESSIONS {
            break;
        }
        let child = entry.path();
        if !child.is_dir() {
            continue;
        }
        let nested = nested_session_dirs(&child);
        if nested.is_empty() {
            push_session(profile, &child, &mut found);
        } else {
            for dir in nested {
                if found.len() >= MAX_SESSIONS {
                    break;
                }
                push_session(profile, &dir, &mut found);
            }
        }
    }
    found
}

fn nested_session_dirs(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

fn push_session(profile: &ProviderProfile, dir: &Path, out: &mut Vec<Session>) {
    let Some(name) = dir.file_name().and_then(|n| n.to_str()) else {
        return;
    };
    if name.starts_with('.') {
        return;
    }
    let updated = fs::metadata(dir)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|| "0".into());
    out.push(Session {
        id: format!("{}:{name}", profile.id),
        provider: profile.id.clone(),
        native_session_id: Some(name.to_string()),
        title: name.to_string(),
        status: SessionStatus::Idle,
        ownership: Ownership::Indexed,
        process_id: None,
        working_dir: None,
        last_user_message: None,
        last_reply_summary: None,
        last_error: None,
        created_at: updated.clone(),
        updated_at: updated,
        url: profile.mcp_url.clone(),
    });
}
