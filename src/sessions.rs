use async_openai::types::{
    ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
    ChatCompletionResponseMessage,
};
use std::fs;

fn get_session_path(name: &str) -> std::path::PathBuf {
    let session_dir = "~/.ask/sessions";

    let session_dir: std::path::PathBuf = shellexpand::tilde(&session_dir)
        .into_owned()
        .parse()
        .unwrap();

    if !&session_dir.exists() {
        std::fs::create_dir_all(&session_dir).unwrap();
    }

    let session_path = format!("{}/{name}", session_dir.display());

    let session_path: std::path::PathBuf = shellexpand::tilde(&session_path)
        .into_owned()
        .parse()
        .unwrap();
    session_path
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
    request: &Vec<ChatCompletionRequestMessage>,
    res: Option<&ChatCompletionResponseMessage>
) -> Result<(), std::io::Error> {
    let session_path = get_session_path(name);
    let mut session = request.clone();
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
    Ok(())
}
