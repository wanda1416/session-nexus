use serde::{Deserialize, Serialize};

/// Our id. Agent-native ids stay in [`Session::native_session_id`].
pub type NexusId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Idle,
    Working,
    WaitingInput,
    WaitingApproval,
    Error,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ownership {
    /// We started the process and must stop its tree on shutdown.
    Owned,
    /// Live process the user already had. Observe only.
    Attached,
    /// Indexed from the provider's session files. No process.
    Indexed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: NexusId,
    pub provider: String,
    pub native_session_id: Option<String>,
    pub title: String,
    pub status: SessionStatus,
    pub ownership: Ownership,
    pub process_id: Option<u32>,
    pub working_dir: Option<String>,
    pub url: Option<String>,
    pub last_user_message: Option<String>,
    pub last_reply_summary: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionOptions {
    pub provider: String,
    pub title: Option<String>,
    pub working_dir: Option<String>,
    pub initial_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    Upsert { session: Session },
    Status { id: NexusId, status: SessionStatus },
    TranscriptAppend { id: NexusId, text: String },
    ApprovalRequired { id: NexusId, prompt: String },
    ProcessExit { id: NexusId, code: Option<i32> },
}
