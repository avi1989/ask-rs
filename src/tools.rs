pub(crate) mod mcp;

use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, Serialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub working_directory: String,
}

pub fn execute_command(command: &str, working_directory: &str) -> String {
    let shell_kind = crate::shell::detect_shell_kind();

    let (shell, flag) = if shell_kind == "Powershell" && cfg!(windows) {
        ("powershell", "-Command")
    } else if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    let output = std::process::Command::new(shell)
        .arg(flag)
        .arg(command)
        .current_dir(working_directory)
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if stderr.is_empty() {
                stdout
            } else {
                format!("stdout:\n{stdout}\n---\nstderr:\n{stderr}")
            }
        }
        Err(e) => format!("Failed to execute command '{command}': {e}"),
    }
}

pub fn execute_command_tool() -> ChatCompletionTool {
    ChatCompletionTool {
        r#type: ChatCompletionToolType::Function,
        function: FunctionObject {
            name: "execute_command".to_string(),
            description: Some("Execute a command on the Operating System".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The command to be executed"},
                    "working_directory": {"type": "string", "description": "The working directory for the command execution (optional)"}
                },
                "required": ["command", "working_directory"]
            })),
            strict: None,
        },
    }
}
