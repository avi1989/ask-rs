use openai_api_rs::v1::chat_completion::Tool;
use openai_api_rs::v1::{chat_completion, types};
use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ListFilesRequest {
    pub base_path: String,
    pub recursive: bool,
}

#[derive(Deserialize, Serialize)]
pub struct ListFilesToolRequest {
    pub file_path: String,
}

#[derive(Deserialize, Serialize)]
pub struct ExecuteCommandRequest {
    pub command: String,
    pub working_directory: String,
}

pub fn list_all_files(base_path: &str, recursive: bool) -> Vec<String> {
    if !recursive {
        return fs::read_dir(base_path)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
    }

    let mut result = Vec::new();
    list_files_recursive(base_path, &mut result).unwrap_or_else(|err| {
        println!("Error listing files recursively: {err}");
    });

    result
}

fn list_files_recursive(path: &str, files: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path_buf = entry.path();

        // Add the file or directory path relative to the starting point
        let path_str = path_buf.to_string_lossy().to_string();
        files.push(path_str);

        // Recursively process subdirectories
        if path_buf.is_dir() {
            list_files_recursive(path_buf.to_str().unwrap(), files)?;
        }
    }

    Ok(())
}



pub fn list_all_files_tool() -> Tool {
    let mut tool_props = HashMap::new();
    tool_props.insert(
        "base_path".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The path to get files from".to_string()),
            ..Default::default()
        }),
    );

    tool_props.insert(
        "recursive".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("A boolean to indicate if all files should be listed recursively".to_string()),
            ..Default::default()
        })
    );

    Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("list_all_files"),
            description: Some(String::from("List all files in a directory")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(tool_props),
                required: Some(vec!["base_path".to_string(), "recursive".to_string()]),
            },
        },
    }
}



pub fn read_file(file_path: &str) -> String {
    fs::read_to_string(file_path).unwrap()
}



pub fn execute_command(command: &str, working_directory: &str) -> String {
    // Detect shell based on platform and environment
    let shell_kind = crate::shell::detect_shell_kind();

    // Configure the shell command based on the detected shell
    let (shell, flag) = if shell_kind == "Powershell" && cfg!(windows) {
        ("powershell", "-Command")
    } else if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };

    // Run the command through the appropriate shell
    let output = std::process::Command::new(shell)
        .arg(flag)
        .arg(command)
        .current_dir(working_directory)
        .output();

    // Process the command output
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if stderr.is_empty() {
                stdout
            } else {
                format!("stdout:\n{stdout}\n---\nstderr:\n{stderr}")
            }
        },
        Err(e) => format!("Failed to execute command '{command}': {e}")
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
            description: Some("The working directory for the command execution (optional)".to_string()),
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
                required: Some(vec!["command".to_string(), "working_directory".to_string()])
            }
        }
    }
}

pub fn read_file_tool() -> Tool {
    let mut tool_props = HashMap::new();
    tool_props.insert(
        "file_path".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The path to the file to read".to_string()),
            ..Default::default()
        }),
    );

    Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("read_file"),
            description: Some(String::from("Read a file")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(tool_props),
                required: Some(vec!["file_path".to_string()]),
            },
        }
    }
}
