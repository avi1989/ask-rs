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
    let a = chrono::Local::now() - datetime;
    if a.num_hours() < 6 {
        if a.num_hours() > 1 {
            return format!("{} hours ago", a.num_hours());
        } else {
            if a.num_minutes() < 1 {
                return "just now".to_string();
            }
            return format!("{} minutes ago", a.num_minutes());
        }
    } else if a.num_hours() < 24 {
        return format!("{} hours ago", a.num_hours());
    }
    datetime.format("%d %b %y %k:%M %p").to_string()
}

fn get_session_dir() -> std::path::PathBuf {
    let session_dir = "~/.ask/sessions";

    let session_dir: std::path::PathBuf = shellexpand::tilde(&session_dir)
        .into_owned()
        .parse()
        .unwrap();

    if !&session_dir.exists() {
        std::fs::create_dir_all(&session_dir).unwrap();
    }

    session_dir
}

fn get_session_path(name: &str) -> std::path::PathBuf {
    let session_dir = get_session_dir();
    let session_path = format!("{}/{name}", session_dir.display());

    let session_path: std::path::PathBuf = shellexpand::tilde(&session_path)
        .into_owned()
        .parse()
        .unwrap();
    session_path
}

pub fn get_all_sessions() -> Vec<Session> {
    let session_dir = get_session_dir();
    let sessions = fs::read_dir(session_dir).unwrap();
    let mut result = vec![];
    for session in sessions {
        let session = session.unwrap();
        let name = session.file_name();
        let modified_date = session.metadata().unwrap().modified().unwrap();
        if name != ".last-session" && name != "last" {
            let s = Session {
                name: name.to_str().unwrap().to_string(),
                created: system_time_to_string(modified_date),
            };

            result.push(s);
        }
    }

    result
}

pub fn get_session(name: &str) -> Option<Vec<ChatCompletionRequestMessage>> {
    let session_path = get_session_path(name);

    if session_path.exists() {
        let session_file = std::fs::File::open(session_path).unwrap();
        let session_data = serde_json::from_reader(session_file);

        match session_data {
            Ok(data) => {
                return data;
            }
            Err(e) => {
                eprintln!("Failed to load session: {e}");
            }
        }
    } else {
        println!("Session not found: {:?}", session_path);
    }

    None
}

pub fn save_session(
    name: &str,
    request: &[ChatCompletionRequestMessage],
    res: Option<&ChatCompletionResponseMessage>,
) -> Result<(), std::io::Error> {
    let session_path = get_session_path(name);
    let mut session = request.to_owned();
    if let Some(res) = res {
        session.push(ChatCompletionRequestMessage::Assistant(
            ChatCompletionRequestAssistantMessage {
                content: res.clone().content.map(|c| c.into()),
                ..Default::default()
            },
        ));
    }
    let session_json = serde_json::to_string_pretty(&session)?;

    fs::write(session_path, session_json)?;
    set_last_session_name(name);
    Ok(())
}

pub fn get_last_session_name() -> Option<String> {
    let session_path = get_session_path(".last-session");
    fs::read_to_string(session_path).ok()
}

fn set_last_session_name(name: &str) {
    let session_path = get_session_path(".last-session");
    fs::write(session_path, name).unwrap();
}
