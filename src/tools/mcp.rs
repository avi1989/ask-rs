use openai_api_rs::v1::chat_completion::Tool;
use openai_api_rs::v1::{chat_completion, types};
use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, ServiceExt};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde_json::Value;
use std::collections::HashMap;
use tokio::process::Command;

type McpService = rmcp::service::RunningService<RoleClient, ()>;

pub struct McpRegistry {
    servers: HashMap<String, McpServerConfig>,
    services: HashMap<String, McpService>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            services: HashMap::new(),
        }
    }

    pub fn from_servers(servers: Vec<(String, McpServerConfig)>) -> Self {
        Self {
            servers: servers.into_iter().collect(),
            services: HashMap::new(),
        }
    }

    pub async fn initialize_services(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for (name, config) in &self.servers {
            let service = create_mcp_service(config).await?;
            self.services.insert(name.clone(), service);
        }
        Ok(())
    }

    pub fn find_server_for_tool(&self, tool_name: &str) -> Option<(&str, &McpServerConfig)> {
        // Tool names are formatted as "{prefix}_{actual_tool_name}"
        for (name, config) in &self.servers {
            let prefix_with_underscore = format!("{}_", config.tool_prefix);
            if tool_name.starts_with(&prefix_with_underscore) {
                return Some((name, config));
            }
        }
        None
    }

    pub fn get_service(&self, server_name: &str) -> Option<&McpService> {
        self.services.get(server_name)
    }

    pub fn servers(&self) -> &HashMap<String, McpServerConfig> {
        &self.servers
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub tool_prefix: String,
}

async fn create_mcp_service(config: &McpServerConfig) -> Result<McpService, Box<dyn std::error::Error>> {
    let command = config.command.clone();
    let args = config.args.clone();
    let env = config.env.clone();

    let service = ()
        .serve(TokioChildProcess::new(Command::new(&command).configure(
            move |cmd| {
                cmd.args(&args);
                cmd.envs(env);
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

pub fn get_mcp_tools(service: &McpService, config: &McpServerConfig) -> Result<Vec<Tool>, String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            match service.list_tools(Default::default()).await {
                Ok(tools_result) => Ok(tools_result
                    .tools
                    .iter()
                    .map(|tool| convert_mcp_tool_to_openai(tool, &config.tool_prefix))
                    .collect()),
                Err(e) => {
                    Err(format!("Failed to list tools: {}", e))
                }
            }
        })
    })
}

pub fn execute_mcp_tool_call(
    service: &McpService,
    config: &McpServerConfig,
    name: &str,
    arguments: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let prefix_with_underscore = format!("{}_", config.tool_prefix);
    let tool_name = name.strip_prefix(&prefix_with_underscore).unwrap_or(name);

    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
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

pub fn load_all_mcp_tools(registry: &McpRegistry, verbose: bool) -> Vec<Tool> {
    let mut all_tools = Vec::new();
    let mut loaded_servers = Vec::new();
    let mut failed_servers = Vec::new();

    for (name, config) in registry.servers() {
        if verbose {
            eprintln!("Loading MCP tools from server '{}'...", name);
        }

        if let Some(service) = registry.get_service(name) {
            match get_mcp_tools(service, config) {
                Ok(tools) => {
                    if verbose {
                        eprintln!("  Loaded {} tools from '{}'", tools.len(), name);
                    }
                    loaded_servers.push((name.clone(), tools.len()));
                    all_tools.extend(tools);
                }
                Err(e) => {
                    eprintln!("Failed to load MCP server '{}': {}", name, e);
                    failed_servers.push(name.clone());
                }
            }
        } else {
            eprintln!("Failed to load MCP server '{}': service not initialized", name);
            failed_servers.push(name.clone());
        }
    }

    if !loaded_servers.is_empty() && !verbose {
        let total_tools: usize = loaded_servers.iter().map(|(_, count)| count).sum();
        let server_names: Vec<&str> = loaded_servers.iter().map(|(name, _)| name.as_str()).collect();
        eprintln!("Loaded {} tools from {} MCP server(s): {}",
                  total_tools,
                  loaded_servers.len(),
                  server_names.join(", "));
    }

    all_tools
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
