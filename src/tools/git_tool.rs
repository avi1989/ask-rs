use openai_api_rs::v1::chat_completion::Tool;
use openai_api_rs::v1::{chat_completion, types};
use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, ServiceExt};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command;

type McpService = rmcp::service::RunningService<RoleClient, ()>;

async fn create_mcp_service() -> Result<McpService, Box<dyn std::error::Error>> {
    // Create a new client connected to the git MCP server via uvx
    let service = ()
        .serve(TokioChildProcess::new(Command::new("uvx").configure(
            |cmd| {
                cmd.arg("mcp-server-git");
            },
        ))?)
        .await?;

    Ok(service)
}

fn convert_mcp_tool_to_openai(mcp_tool: &rmcp::model::Tool) -> Tool {
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
            name: format!("git_{}", mcp_tool.name),
            description: mcp_tool.description.as_ref().map(|s| s.to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(required),
            },
        },
    }
}

pub fn git_mcp_tools() -> Vec<Tool> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            match create_mcp_service().await {
                Ok(service) => match service.list_tools(Default::default()).await {
                    Ok(tools_result) => tools_result
                        .tools
                        .iter()
                        .map(convert_mcp_tool_to_openai)
                        .collect(),
                    Err(e) => {
                        eprintln!("Failed to list git MCP tools: {}", e);
                        Vec::new()
                    }
                },
                Err(e) => {
                    eprintln!("Failed to connect to git MCP server: {}", e);
                    Vec::new()
                }
            }
        })
    })
}

pub fn execute_git_tool_call(
    name: &str,
    arguments: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Remove "git_" prefix to get the actual MCP tool name
    let tool_name = name.strip_prefix("git_").unwrap_or(name);

    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let service = create_mcp_service().await?;

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
