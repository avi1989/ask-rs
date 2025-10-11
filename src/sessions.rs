use anyhow::{Context, Result};
use async_openai::types::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
    ChatCompletionResponseMessage,
};
use chrono::{DateTime, Local};
use std::fs;
use std::time::SystemTime;

pub struct Session {
    pub name: String,
    pub created: String,
}
fn system_time_to_string(system_time: SystemTime) -> String {
    let datetime: DateTime<Local> = system_time.into();
    let duration = chrono::Local::now() - datetime;

    if duration.num_hours() < 1 {
        if duration.num_minutes() < 1 {
            "just now".to_string()
        } else {
            format!("{} minutes ago", duration.num_minutes())
        }
    } else if duration.num_hours() < 24 {
        format!("{} hours ago", duration.num_hours())
    } else {
        datetime.format("%d %b %y %k:%M %p").to_string()
    }
}

fn get_session_dir() -> Result<std::path::PathBuf> {
    let session_dir = "~/.ask/sessions";

    let session_dir: std::path::PathBuf = shellexpand::tilde(&session_dir)
        .into_owned()
        .parse()
        .context("Failed to parse session directory path")?;

    if !session_dir.exists() {
        fs::create_dir_all(&session_dir).context(format!(
            "Failed to create session directory at {:?}",
            session_dir
        ))?;
    }

    Ok(session_dir)
}

fn get_session_path(name: &str) -> Result<std::path::PathBuf> {
    let session_dir = get_session_dir()?;
    let session_path = format!("{}/{name}", session_dir.display());

    let session_path: std::path::PathBuf =
        shellexpand::tilde(&session_path)
            .into_owned()
            .parse()
            .context(format!("Failed to parse session path for '{}'", name))?;

    Ok(session_path)
}

pub fn get_all_sessions() -> Result<Vec<Session>> {
    let session_dir = get_session_dir()?;
    let sessions = fs::read_dir(&session_dir).context(format!(
        "Failed to read session directory at {:?}",
        session_dir
    ))?;

    let mut result = vec![];
    for entry in sessions {
        let entry = entry.context("Failed to read session directory entry")?;
        let name = entry.file_name();

        // Skip hidden session files
        if name == ".last-session" || name == "last" {
            continue;
        }

        let metadata = entry
            .metadata()
            .context(format!("Failed to read metadata for session {:?}", name))?;
        let modified_date = metadata.modified().context(format!(
            "Failed to get modified date for session {:?}",
            name
        ))?;

        if let Some(name_str) = name.to_str() {
            result.push(Session {
                name: name_str.to_string(),
                created: system_time_to_string(modified_date),
            });
        }
    }

    Ok(result)
}

pub fn get_session(name: &str) -> Option<Vec<ChatCompletionRequestMessage>> {
    let session_path = match get_session_path(name) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Failed to get session path for '{}': {}", name, e);
            return None;
        }
    };

    if !session_path.exists() {
        eprintln!("Session not found: {:?}", session_path);
        return None;
    }

    let session_file = match fs::File::open(&session_path) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Failed to open session file {:?}: {}", session_path, e);
            return None;
        }
    };

    match serde_json::from_reader(session_file) {
        Ok(data) => Some(data),
        Err(e) => {
            eprintln!("Failed to parse session '{}': {}", name, e);
            None
        }
    }
}

pub fn save_session(
    name: &str,
    request: &[ChatCompletionRequestMessage],
    res: Option<&ChatCompletionResponseMessage>,
) -> Result<()> {
    let session_path = get_session_path(name)?;

    let mut session = request.to_owned();
    if let Some(res) = res {
        session.push(ChatCompletionRequestMessage::Assistant(
            ChatCompletionRequestAssistantMessage {
                content: res.clone().content.map(|c| c.into()),
                ..Default::default()
            },
        ));
    }

    let session_json =
        serde_json::to_string_pretty(&session).context("Failed to serialize session to JSON")?;

    fs::write(&session_path, session_json)
        .context(format!("Failed to write session to {:?}", session_path))?;

    set_last_session_name(name)?;
    Ok(())
}

pub fn get_last_session_name() -> Option<String> {
    let session_path = get_session_path(".last-session").ok()?;
    fs::read_to_string(session_path).ok()
}

fn set_last_session_name(name: &str) -> Result<()> {
    let session_path = get_session_path(".last-session")?;
    fs::write(&session_path, name).context(format!(
        "Failed to write last session name to {:?}",
        session_path
    ))?;
    Ok(())
}
