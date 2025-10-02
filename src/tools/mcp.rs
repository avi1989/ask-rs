//! Generic MCP (Model Context Protocol) tool integration
//!
//! This module provides a generic way to integrate any MCP server with OpenAI-compatible tools.
//!
//! # Adding a new MCP server
//!
//! 1. Create a configuration:
//! ```
//! let config = McpServerConfig::new(
//!     "uvx",                              // Command to run
//!     vec!["mcp-server-name".to_string()], // Arguments
//!     "prefix"                             // Tool name prefix
//! );
//! ```
//!
//! 2. Get tools:
//! ```
//! let tools = get_mcp_tools(&config);
//! ```
//!
//! 3. Execute tool calls:
//! ```
//! let result = execute_mcp_tool_call(&config, "prefix_tool_name", "{\"arg\": \"value\"}");
//! ```

use openai_api_rs::v1::chat_completion::Tool;
use openai_api_rs::v1::{chat_completion, types};
use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, ServiceExt};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command;

type McpService = rmcp::service::RunningService<RoleClient, ()>;

/// Configuration for an MCP server
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub tool_prefix: String,
}

impl McpServerConfig {
    pub fn new(command: impl Into<String>, args: Vec<String>, tool_prefix: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args,
            tool_prefix: tool_prefix.into(),
        }
    }
}

async fn create_mcp_service(config: &McpServerConfig) -> Result<McpService, Box<dyn std::error::Error>> {
    let command = config.command.clone();
    let args = config.args.clone();

    let service = ()
        .serve(TokioChildProcess::new(Command::new(&command).configure(
            move |cmd| {
                cmd.args(&args);
            },
        ))?)
        .await?;

    Ok(service)
}

fn convert_mcp_tool_to_openai(mcp_tool: &rmcp::model::Tool, prefix: &str) -> Tool {
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    let input_schema = &mcp_tool.input_schema;
    if let Some(props) = input_schema.get("properties").and_then(|v| v.as_object()) {
        for (key, value) in props {
            let schema_type = value
                .get("type")
                .and_then(|t| t.as_str())
                .and_then(|t| match t {
                    "string" => Some(types::JSONSchemaType::String),
                    "number" | "integer" => Some(types::JSONSchemaType::Number),
                    "boolean" => Some(types::JSONSchemaType::Boolean),
                    "array" => Some(types::JSONSchemaType::Array),
                    "object" => Some(types::JSONSchemaType::Object),
                    _ => None,
                });

            // Skip properties without a valid schema type
            let Some(schema_type) = schema_type else {
                continue;
            };

            let description = value
                .get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());

            // Handle array items if this is an array type
            let items = if matches!(schema_type, types::JSONSchemaType::Array) {
                value.get("items").and_then(|items_value| {
                    let item_type = items_value
                        .get("type")
                        .and_then(|t| t.as_str())
                        .and_then(|t| match t {
                            "string" => Some(types::JSONSchemaType::String),
                            "number" | "integer" => Some(types::JSONSchemaType::Number),
                            "boolean" => Some(types::JSONSchemaType::Boolean),
                            "array" => Some(types::JSONSchemaType::Array),
                            "object" => Some(types::JSONSchemaType::Object),
                            _ => None,
                        });

                    let item_description = items_value
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());

                    Some(Box::new(types::JSONSchemaDefine {
                        schema_type: item_type,
                        description: item_description,
                        ..Default::default()
                    }))
                })
            } else {
                None
            };

            properties.insert(
                key.clone(),
                Box::new(types::JSONSchemaDefine {
                    schema_type: Some(schema_type),
                    description,
                    items,
                    ..Default::default()
                }),
            );
        }
    }

    if let Some(req) = input_schema.get("required").and_then(|v| v.as_array()) {
        for item in req {
            if let Some(s) = item.as_str() {
                required.push(s.to_string());
            }
        }
    }

    Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: format!("{}_{}", prefix, mcp_tool.name),
            description: mcp_tool.description.as_ref().map(|s| s.to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(required),
            },
        },
    }
}

pub fn get_mcp_tools(config: &McpServerConfig) -> Vec<Tool> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            match create_mcp_service(config).await {
                Ok(service) => match service.list_tools(Default::default()).await {
                    Ok(tools_result) => tools_result
                        .tools
                        .iter()
                        .map(|tool| convert_mcp_tool_to_openai(tool, &config.tool_prefix))
                        .collect(),
                    Err(e) => {
                        eprintln!("Failed to list MCP tools for {}: {}", config.tool_prefix, e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    eprintln!("Failed to connect to MCP server {}: {}", config.tool_prefix, e);
                    Vec::new()
                }
            }
        })
    })
}

pub fn execute_mcp_tool_call(
    config: &McpServerConfig,
    name: &str,
    arguments: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Remove prefix to get the actual MCP tool name
    let prefix_with_underscore = format!("{}_", config.tool_prefix);
    let tool_name = name.strip_prefix(&prefix_with_underscore).unwrap_or(name);

    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let service = create_mcp_service(config).await?;

            let args: Value = serde_json::from_str(arguments)?;
            let args_object = args.as_object().cloned();

            let result = service
                .call_tool(CallToolRequestParam {
                    name: tool_name.to_string().into(),
                    arguments: args_object,
                })
                .await?;

            Ok(format_tool_result(&result))
        })
    })
}

// Git-specific convenience functions
pub fn git_mcp_tools() -> Vec<Tool> {
    let config = McpServerConfig::new("uvx", vec!["mcp-server-git".to_string()], "git");
    get_mcp_tools(&config)
}

pub fn execute_git_tool_call(
    name: &str,
    arguments: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let config = McpServerConfig::new("uvx", vec!["mcp-server-git".to_string()], "git");
    execute_mcp_tool_call(&config, name, arguments)
}

fn format_tool_result(result: &rmcp::model::CallToolResult) -> String {
    let mut output = String::new();

    for content in &result.content {
        match &content.raw {
            rmcp::model::RawContent::Text(text_content) => {
                output.push_str(&text_content.text);
                output.push('\n');
            }
            rmcp::model::RawContent::Image(image_content) => {
                output.push_str(&format!(
                    "[Image: {} ({} bytes)]\n",
                    image_content.mime_type,
                    image_content.data.len()
                ));
            }
            rmcp::model::RawContent::Resource(embedded_resource) => {
                // Format the resource contents
                output.push_str(&format!("[Resource: {:?}]\n", embedded_resource.resource));
            }
            rmcp::model::RawContent::Audio(audio_content) => {
                output.push_str(&format!(
                    "[Audio: {} ({} bytes)]\n",
                    audio_content.mime_type,
                    audio_content.data.len()
                ));
            }
            rmcp::model::RawContent::ResourceLink(resource_link) => {
                output.push_str(&format!("[Resource: {}]\n", resource_link.uri));
            }
        }
    }

    if result.is_error.unwrap_or(false) {
        format!("Error: {}", output)
    } else {
        output
    }
}
