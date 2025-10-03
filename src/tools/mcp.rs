use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use rmcp::model::CallToolRequestParam;
use rmcp::service::{RoleClient, ServiceExt};
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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

    #[allow(dead_code)]
    pub async fn initialize_services(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        use futures::future::join_all;

        // Spawn all MCP servers in parallel
        let mut tasks = Vec::new();
        for (name, config) in &self.servers {
            let config_clone = config.clone();
            let name_clone = name.clone();
            tasks.push(async move {
                let result = create_mcp_service(&config_clone).await;
                (name_clone, result)
            });
        }

        // Wait for all to complete
        let results = join_all(tasks).await;

        // Collect successful services, log failures
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
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.services.contains_key(server_name) {
            return Ok(()); // Already initialized
        }

        if let Some(config) = self.servers.get(server_name) {
            match create_mcp_service(config).await {
                Ok(service) => {
                    self.services.insert(server_name.to_string(), service);

                    // Update cache with tools from this server
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
    fn hash(&self) -> String {
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

#[derive(Serialize, Deserialize)]
struct ToolCache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    config_hash: String,
    tools: Vec<ChatCompletionTool>,
}

async fn create_mcp_service(
    config: &McpServerConfig,
) -> Result<McpService, Box<dyn std::error::Error>> {
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

fn convert_mcp_tool_to_openai(mcp_tool: &rmcp::model::Tool, prefix: &str) -> ChatCompletionTool {
    // Simply pass through the MCP tool's JSON Schema as-is into async-openai's FunctionObject
    let name = format!("{}_{}", prefix, mcp_tool.name);
    ChatCompletionTool {
        r#type: ChatCompletionToolType::Function,
        function: FunctionObject {
            name,
            description: mcp_tool.description.as_ref().map(|c| c.to_string()),
            parameters: Some(serde_json::Value::Object(
                mcp_tool.input_schema.as_ref().clone(),
            )),
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

fn get_cache_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".ask_tools_cache.json")
}

fn load_cache() -> ToolCache {
    let cache_path = get_cache_path();
    if let Ok(content) = fs::read_to_string(&cache_path)
        && let Ok(cache) = serde_json::from_str(&content)
    {
        return cache;
    }
    ToolCache {
        entries: HashMap::new(),
    }
}

fn save_cache(cache: &ToolCache) {
    let cache_path = get_cache_path();
    if let Ok(content) = serde_json::to_string_pretty(cache) {
        let _ = fs::write(&cache_path, content);
    }
}

pub async fn populate_cache_if_needed(
    registry: &mut McpRegistry,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let cache = load_cache();
    let mut missing_servers = Vec::new();

    for (name, config) in registry.servers() {
        let config_hash = config.hash();
        let is_cached = cache
            .entries
            .get(name)
            .map(|entry| entry.config_hash == config_hash)
            .unwrap_or(false);

        if !is_cached {
            missing_servers.push(name.clone());
        }
    }

    if !missing_servers.is_empty() {
        if verbose {
            eprintln!(
                "Building tool cache for {} server(s)...",
                missing_servers.len()
            );
        } else {
            eprintln!("First run: initializing MCP servers to build cache...");
        }

        use futures::future::join_all;
        let mut tasks = Vec::new();

        for name in &missing_servers {
            if let Some(config) = registry.servers().get(name) {
                let config_clone = config.clone();
                let name_clone = name.clone();
                tasks.push(async move {
                    let result = create_mcp_service(&config_clone).await;
                    (name_clone, config_clone, result)
                });
            }
        }

        let results = join_all(tasks).await;

        for (name, config, result) in results {
            match result {
                Ok(service) => {
                    if let Ok(tools) = get_mcp_tools(&service, &config) {
                        update_cache_for_server(&name, &config, tools);
                        if verbose {
                            eprintln!("  Cached tools for '{name}'");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to initialize MCP server '{name}': {e}");
                }
            }
        }

        if !verbose {
            eprintln!("Cache built. Future runs will be faster!");
        }
    }

    Ok(())
}

pub fn load_cached_tools(registry: &McpRegistry, verbose: bool) -> Vec<ChatCompletionTool> {
    let cache = load_cache();
    let mut all_tools = Vec::new();
    let mut loaded_count = 0;

    for (name, config) in registry.servers() {
        let config_hash = config.hash();

        if let Some(entry) = cache.entries.get(name)
            && entry.config_hash == config_hash
        {
            if verbose {
                eprintln!(
                    "Loaded {} tools from cache for '{}'",
                    entry.tools.len(),
                    name
                );
            }
            all_tools.extend(entry.tools.clone());
            loaded_count += 1;
            continue;
        }

        if verbose {
            eprintln!("No cache for '{name}', will initialize on first use");
        }
    }

    if loaded_count > 0 && !verbose {
        eprintln!("Loaded {loaded_count} MCP server(s) from cache");
    }

    all_tools
}

pub fn update_cache_for_server(
    server_name: &str,
    config: &McpServerConfig,
    tools: Vec<ChatCompletionTool>,
) {
    let mut cache = load_cache();
    cache.entries.insert(
        server_name.to_string(),
        CacheEntry {
            config_hash: config.hash(),
            tools,
        },
    );
    save_cache(&cache);
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
