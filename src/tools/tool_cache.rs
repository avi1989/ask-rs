use crate::tools::mcp::{McpServerConfig, get_mcp_tools};
use async_openai::types::ChatCompletionTool;
use rmcp::RoleClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct ToolCache {
    pub entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct CacheEntry {
    pub config_hash: String,
    pub tools: Vec<ChatCompletionTool>,
}

pub type McpService = rmcp::service::RunningService<RoleClient, ()>;

pub struct McpRegistry {
    pub servers: HashMap<String, McpServerConfig>,
    pub services: HashMap<String, McpService>,
}

fn get_cache_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".ask/tools_cache.json")
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
                    let result =
                        crate::tools::mcp::create_mcp_service(&config_clone, verbose).await;
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

    if loaded_count > 0 && verbose {
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
