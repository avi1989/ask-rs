use crate::tools::tool_cache::{McpRegistry, McpService, update_cache_for_server};
use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use rmcp::model::CallToolRequestParam;
use rmcp::service::ServiceExt;
use rmcp::transport::TokioChildProcess;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            services: HashMap::new(),
        }
    }

    pub fn from_servers(servers: Vec<(String, McpServerConfig)>) -> Self {
        for (name, _) in &servers {
            if name.contains(".") {
                eprintln!(
                    "Warning: MCP server name '{name}' contains a '.' which is not allowed. Please rename it."
                );
                std::process::exit(1);
            }
        }
        Self {
            servers: servers.into_iter().collect(),
            services: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub async fn initialize_services(
        &mut self,
        verbose: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use futures::future::join_all;

        // Spawn all MCP servers in parallel
        let mut tasks = Vec::new();
        for (name, config) in &self.servers {
            let config_clone = config.clone();
            let name_clone = name.clone();
            tasks.push(async move {
                let result = create_mcp_service(&config_clone, verbose).await;
                (name_clone, result)
            });
        }

        let results = join_all(tasks).await;

        for (name, result) in results {
            match result {
                Ok(service) => {
                    self.services.insert(name, service);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to initialize MCP server '{name}': {e}");
                }
            }
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

    #[allow(dead_code)]
    pub fn get_server_config(&self, server_name: &str) -> Option<&McpServerConfig> {
        self.servers.get(server_name)
    }

    pub async fn initialize_service(
        &mut self,
        server_name: &str,
        verbose: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.services.contains_key(server_name) {
            return Ok(()); // Already initialized
        }

        if let Some(config) = self.servers.get(server_name) {
            match create_mcp_service(config, verbose).await {
                Ok(service) => {
                    self.services.insert(server_name.to_string(), service);

                    if let Some(service) = self.services.get(server_name)
                        && let Ok(tools) = get_mcp_tools(service, config)
                    {
                        update_cache_for_server(server_name, config, tools);
                    }

                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            Err(format!("Server '{server_name}' not found in registry").into())
        }
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub tool_prefix: String,
}

impl McpServerConfig {
    pub fn hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.command.hash(&mut hasher);
        for arg in &self.args {
            arg.hash(&mut hasher);
        }
        let mut env_vec: Vec<_> = self.env.iter().collect();
        env_vec.sort();
        for (k, v) in env_vec {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        self.tool_prefix.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }
}

pub async fn create_mcp_service(
    config: &McpServerConfig,
    verbose: bool,
) -> Result<McpService, Box<dyn std::error::Error>> {
    let command = config.command.clone();
    let args = config.args.clone();
    let env = config.env.clone();

    // Create command
    let mut cmd = Command::new(&command);
    cmd.args(&args);
    cmd.envs(&env);

    let mut proc = TokioChildProcess::builder(cmd);
    if verbose {
        proc = proc.stderr(Stdio::inherit());
    } else {
        proc = proc.stderr(Stdio::null());
    }

    let (child_process, _stderr) = proc.spawn()?;
    let service = ().serve(child_process).await?;

    Ok(service)
}

fn convert_mcp_tool_to_openai(mcp_tool: &rmcp::model::Tool, prefix: &str) -> ChatCompletionTool {
    let name = format!("{}_{}", prefix, mcp_tool.name);

    // Ensure the schema has required fields for OpenAI
    let mut schema = mcp_tool.input_schema.as_ref().clone();
    if !schema.contains_key("type") {
        schema.insert(
            "type".to_string(),
            serde_json::Value::String("object".to_string()),
        );
    }
    if !schema.contains_key("properties") {
        schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(Default::default()),
        );
    }

    ChatCompletionTool {
        r#type: ChatCompletionToolType::Function,
        function: FunctionObject {
            name,
            description: mcp_tool.description.as_ref().map(|c| c.to_string()),
            parameters: Some(serde_json::Value::Object(schema)),
            strict: None,
        },
    }
}

pub fn get_mcp_tools(
    service: &McpService,
    config: &McpServerConfig,
) -> Result<Vec<ChatCompletionTool>, String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            match service.list_tools(Default::default()).await {
                Ok(tools_result) => Ok(tools_result
                    .tools
                    .iter()
                    .map(|tool| convert_mcp_tool_to_openai(tool, &config.tool_prefix))
                    .collect()),
                Err(e) => Err(format!("Failed to list tools: {e}")),
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

#[allow(dead_code)]
pub fn load_all_mcp_tools(registry: &McpRegistry, verbose: bool) -> Vec<ChatCompletionTool> {
    let mut all_tools = Vec::new();
    let mut loaded_servers = Vec::new();
    let mut failed_servers = Vec::new();

    for (name, config) in registry.servers() {
        if verbose {
            eprintln!("Loading MCP tools from server '{name}'...");
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
                    eprintln!("Failed to load MCP server '{name}': {e}");
                    failed_servers.push(name.clone());
                }
            }
        } else {
            eprintln!("Failed to load MCP server '{name}': service not initialized");
            failed_servers.push(name.clone());
        }
    }

    if !loaded_servers.is_empty() && !verbose {
        let total_tools: usize = loaded_servers.iter().map(|(_, count)| count).sum();
        let server_names: Vec<&str> = loaded_servers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        eprintln!(
            "Loaded {} tools from {} MCP server(s): {}",
            total_tools,
            loaded_servers.len(),
            server_names.join(", ")
        );
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
        format!("Error: {output}")
    } else {
        output
    }
}
