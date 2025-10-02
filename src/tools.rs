pub(crate) mod mcp;

use openai_api_rs::v1::chat_completion::Tool;
use openai_api_rs::v1::{chat_completion, types};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

pub fn execute_command_tool() -> Tool {
    let mut tool_props = HashMap::new();
    tool_props.insert(
        "command".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The command to be executed".to_string()),
            ..Default::default()
        }),
    );
    tool_props.insert(
        "working_directory".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "The working directory for the command execution (optional)".to_string(),
            ),
            ..Default::default()
        }),
    );

    Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("execute_command"),
            description: Some(String::from("Execute a command on the Operating System")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(tool_props),
                required: Some(vec!["command".to_string(), "working_directory".to_string()]),
            },
        },
    }
}
