use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMessage {
    pub id: String,
    pub kind: String,
    pub contact_id: Option<String>,
    pub text: String,
    pub work_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredWork {
    pub id: String,
    pub contact_id: String,
    pub task: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRoom {
    pub contact_id: String,
    pub native_session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContactProfile {
    pub contact_id: String,
    pub model: String,
    pub api_key: String,
    pub env_text: String,
    pub persona: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredGroup {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub brief: String,
    pub member_ids: Vec<String>,
    pub messages: Vec<StoredMessage>,
    pub works: Vec<StoredWork>,
    #[serde(default)]
    pub room_sessions: Vec<StoredRoom>,
}

pub struct ChatDb {
    conn: Mutex<Connection>,
}

impl ChatDb {
    pub fn open() -> Result<Self, String> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let conn = Connection::open(&path).map_err(|err| err.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                cwd TEXT NOT NULL,
                brief TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS members (
                group_id TEXT NOT NULL,
                contact_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (group_id, contact_id)
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                contact_id TEXT,
                text TEXT NOT NULL,
                work_id TEXT,
                position INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS works (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL,
                contact_id TEXT NOT NULL,
                task TEXT NOT NULL,
                status TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS room_sessions (
                group_id TEXT NOT NULL,
                contact_id TEXT NOT NULL,
                native_session_id TEXT NOT NULL,
                PRIMARY KEY (group_id, contact_id)
            );
            CREATE TABLE IF NOT EXISTS contact_profiles (
                contact_id TEXT PRIMARY KEY,
                model TEXT NOT NULL DEFAULT '',
                api_key TEXT NOT NULL DEFAULT '',
                env_text TEXT NOT NULL DEFAULT '',
                persona TEXT NOT NULL DEFAULT ''
            );",
        )
        .map_err(|err| err.to_string())?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn load(&self) -> Result<Vec<StoredGroup>, String> {
        let conn = self.conn.lock().map_err(|err| err.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, name, cwd, brief FROM groups ORDER BY rowid")
            .map_err(|err| err.to_string())?;
        let groups = stmt
            .query_map([], |row| {
                Ok(StoredGroup {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    cwd: row.get(2)?,
                    brief: row.get(3)?,
                    member_ids: Vec::new(),
                    messages: Vec::new(),
                    works: Vec::new(),
                    room_sessions: Vec::new(),
                })
            })
            .map_err(|err| err.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())?;
        drop(stmt);
        let mut full = Vec::new();
        for mut group in groups {
            group.member_ids = query_strings(
                &conn,
                "SELECT contact_id FROM members WHERE group_id = ?1 ORDER BY position",
                &group.id,
            )?;
            group.messages = query_messages(&conn, &group.id)?;
            group.works = query_works(&conn, &group.id)?;
            group.room_sessions = query_rooms(&conn, &group.id)?;
            full.push(group);
        }
        Ok(full)
    }

    pub fn save(&self, group: &StoredGroup) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|err| err.to_string())?;
        let tx = conn.unchecked_transaction().map_err(|err| err.to_string())?;
        tx.execute(
            "INSERT INTO groups (id, name, cwd, brief) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, cwd = excluded.cwd, brief = excluded.brief",
            params![group.id, group.name, group.cwd, group.brief],
        )
        .map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM members WHERE group_id = ?1", [&group.id])
            .map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM messages WHERE group_id = ?1", [&group.id])
            .map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM works WHERE group_id = ?1", [&group.id])
            .map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM room_sessions WHERE group_id = ?1", [&group.id])
            .map_err(|err| err.to_string())?;
        for (index, contact_id) in group.member_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO members (group_id, contact_id, position) VALUES (?1, ?2, ?3)",
                params![group.id, contact_id, index as i64],
            )
            .map_err(|err| err.to_string())?;
        }
        for (index, message) in group.messages.iter().enumerate() {
            tx.execute(
                "INSERT INTO messages (id, group_id, kind, contact_id, text, work_id, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    message.id,
                    group.id,
                    message.kind,
                    message.contact_id,
                    message.text,
                    message.work_id,
                    index as i64
                ],
            )
            .map_err(|err| err.to_string())?;
        }
        for room in &group.room_sessions {
            tx.execute(
                "INSERT INTO room_sessions (group_id, contact_id, native_session_id) VALUES (?1, ?2, ?3)",
                params![group.id, room.contact_id, room.native_session_id],
            )
            .map_err(|err| err.to_string())?;
        }
        for work in &group.works {
            tx.execute(
                "INSERT INTO works (id, group_id, contact_id, task, status) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![work.id, group.id, work.contact_id, work.task, work.status],
            )
            .map_err(|err| err.to_string())?;
        }
        tx.commit().map_err(|err| err.to_string())
    }

    pub fn load_profiles(&self) -> Result<Vec<ContactProfile>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁失效".to_string())?;
        let mut stmt = conn
            .prepare("SELECT contact_id, model, api_key, env_text, persona FROM contact_profiles")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ContactProfile {
                    contact_id: row.get(0)?,
                    model: row.get(1)?,
                    api_key: row.get(2)?,
                    env_text: row.get(3)?,
                    persona: row.get(4)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
    }

    pub fn get_profile(&self, contact_id: &str) -> Result<Option<ContactProfile>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁失效".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT contact_id, model, api_key, env_text, persona FROM contact_profiles WHERE contact_id = ?1",
            )
            .map_err(|err| err.to_string())?;
        let mut rows = stmt
            .query_map([contact_id], |row| {
                Ok(ContactProfile {
                    contact_id: row.get(0)?,
                    model: row.get(1)?,
                    api_key: row.get(2)?,
                    env_text: row.get(3)?,
                    persona: row.get(4)?,
                })
            })
            .map_err(|err| err.to_string())?;
        match rows.next() {
            Some(row) => row.map(Some).map_err(|err| err.to_string()),
            None => Ok(None),
        }
    }

    pub fn save_profile(&self, profile: &ContactProfile) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁失效".to_string())?;
        conn.execute(
            "INSERT INTO contact_profiles (contact_id, model, api_key, env_text, persona)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(contact_id) DO UPDATE SET
               model = excluded.model,
               api_key = excluded.api_key,
               env_text = excluded.env_text,
               persona = excluded.persona",
            params![
                profile.contact_id,
                profile.model,
                profile.api_key,
                profile.env_text,
                profile.persona,
            ],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn db_path() -> Result<PathBuf, String> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join("AppData").join("Roaming")))
        .ok_or_else(|| "找不到 APPDATA".to_string())?;
    Ok(base.join("SessionNexus").join("nexus.db"))
}

fn query_strings(conn: &Connection, sql: &str, group_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([group_id], |row| row.get(0))
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

fn query_messages(conn: &Connection, group_id: &str) -> Result<Vec<StoredMessage>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, kind, contact_id, text, work_id FROM messages
             WHERE group_id = ?1 ORDER BY position",
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([group_id], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                kind: row.get(1)?,
                contact_id: row.get(2)?,
                text: row.get(3)?,
                work_id: row.get(4)?,
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

fn query_rooms(conn: &Connection, group_id: &str) -> Result<Vec<StoredRoom>, String> {
    let mut stmt = conn
        .prepare("SELECT contact_id, native_session_id FROM room_sessions WHERE group_id = ?1")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([group_id], |row| {
            Ok(StoredRoom {
                contact_id: row.get(0)?,
                native_session_id: row.get(1)?,
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

fn query_works(conn: &Connection, group_id: &str) -> Result<Vec<StoredWork>, String> {
    let mut stmt = conn
        .prepare("SELECT id, contact_id, task, status FROM works WHERE group_id = ?1 ORDER BY rowid")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([group_id], |row| {
            Ok(StoredWork {
                id: row.get(0)?,
                contact_id: row.get(1)?,
                task: row.get(2)?,
                status: row.get(3)?,
            })
        })
        .map_err(|err| err.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_group_messages_and_works() {
        let path = std::env::temp_dir().join(format!("nexus-store-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        let db = ChatDb {
            conn: Mutex::new(conn),
        };
        conn_init(&db);
        let group = StoredGroup {
            id: "group-a".into(),
            name: "项目 A".into(),
            cwd: "D:\\work\\project-a".into(),
            brief: "说明".into(),
            member_ids: vec!["grok".into(), "dsh".into()],
            messages: vec![StoredMessage {
                id: "m1".into(),
                kind: "user".into(),
                contact_id: None,
                text: "@Grok 初始化".into(),
                work_id: None,
            }],
            works: vec![StoredWork {
                id: "work-1".into(),
                contact_id: "grok".into(),
                task: "初始化".into(),
                status: "Working".into(),
            }],
            room_sessions: vec![StoredRoom {
                contact_id: "grok".into(),
                native_session_id: "room-1".into(),
            }],
        };
        db.save(&group).unwrap();
        let loaded = db.load().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].member_ids, group.member_ids);
        assert_eq!(loaded[0].messages[0].text, "@Grok 初始化");
        assert_eq!(loaded[0].works[0].status, "Working");
        assert_eq!(loaded[0].room_sessions[0].native_session_id, "room-1");
    }

    fn conn_init(db: &ChatDb) {
        db.conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TABLE groups (id TEXT PRIMARY KEY, name TEXT NOT NULL, cwd TEXT NOT NULL, brief TEXT NOT NULL);
                 CREATE TABLE members (group_id TEXT NOT NULL, contact_id TEXT NOT NULL, position INTEGER NOT NULL, PRIMARY KEY (group_id, contact_id));
                 CREATE TABLE messages (id TEXT PRIMARY KEY, group_id TEXT NOT NULL, kind TEXT NOT NULL, contact_id TEXT, text TEXT NOT NULL, work_id TEXT, position INTEGER NOT NULL);
                 CREATE TABLE works (id TEXT PRIMARY KEY, group_id TEXT NOT NULL, contact_id TEXT NOT NULL, task TEXT NOT NULL, status TEXT NOT NULL);
                 CREATE TABLE room_sessions (group_id TEXT NOT NULL, contact_id TEXT NOT NULL, native_session_id TEXT NOT NULL, PRIMARY KEY (group_id, contact_id));",
            )
            .unwrap();
    }
}
