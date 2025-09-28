use openai_api_rs::v1::chat_completion::Tool;
use openai_api_rs::v1::{chat_completion, types};
use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ListFilesRequest {
    pub base_path: String,
}

#[derive(Deserialize, Serialize)]
pub struct ListFilesToolRequest {
    pub file_path: String,
}

pub fn list_all_files(base_path: &str) -> Vec<String> {
    fs::read_dir(base_path)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect()
}

pub fn read_file(file_path: &str) -> String {
    fs::read_to_string(file_path).unwrap()
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

    Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("list_all_files"),
            description: Some(String::from("List all files in a directory")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(tool_props),
                required: Some(vec!["base_path".to_string()]),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_list_all_files() {
        let files = list_all_files(".");
        assert!(files.len() > 0);
    }
}
