//! Configuration file loading for MCP servers
//!
//! Loads MCP server configurations from `~/.ask/config` using Claude Code's `.mcp.json` format.

use crate::tools::mcp::McpServerConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AskConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerDefinition>,

    #[serde(rename = "autoApprovedTools", default)]
    pub auto_approved_tools: Vec<String>,

    #[serde(rename = "baseUrl", default)]
    pub base_url: Option<String>,

    #[serde(rename = "defaultModel", default)]
    pub model: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpServerDefinition {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

pub fn load_config() -> Result<AskConfig, Box<dyn std::error::Error>> {
    let config_path = find_config_file()?;
    let contents = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file {config_path:?}: {e}"))?;

    let config: AskConfig = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse config file {config_path:?}: {e}"))?;

    Ok(config)
}

fn find_config_file() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home_config: PathBuf = shellexpand::tilde("~/.ask/config").into_owned().parse()?;
    if home_config.exists() {
        return Ok(home_config);
    }

    Err("No configuration file found. Create ~/.ask/config".into())
}

pub fn config_to_servers(config: &AskConfig) -> Vec<(String, McpServerConfig)> {
    config
        .mcp_servers
        .iter()
        .map(|(name, def)| {
            let server_config = McpServerConfig {
                command: expand_env_vars(&def.command),
                args: def.args.iter().map(|arg| expand_env_vars(arg)).collect(),
                env: def
                    .env
                    .iter()
                    .map(|(k, v)| (k.clone(), expand_env_vars(v)))
                    .collect(),
                tool_prefix: name.clone(),
            };
            (name.clone(), server_config)
        })
        .collect()
}

pub fn save_config(config: &AskConfig) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_path: PathBuf = shellexpand::tilde("~/.ask/config").into_owned().parse()?;
    let config_dir = config_path.parent().unwrap();
    if !config_dir.exists() {
        fs::create_dir_all(config_dir)
            .map_err(|e| format!("Failed to create config directory {config_dir:?}: {e}"))?;
    }

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;

    fs::write(&config_path, json)
        .map_err(|e| format!("Failed to write config to {config_path:?}: {e}"))?;

    Ok(config_path)
}

pub fn add_server(
    name: &str,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config = load_config().unwrap_or_else(|_| AskConfig {
        mcp_servers: HashMap::new(),
        auto_approved_tools: Vec::new(),
        base_url: None,
        model: None,
    });

    if config.mcp_servers.contains_key(name) {
        return Err(format!(
            "Server '{name}' already exists. Remove it first with: ask-rs remove {name}"
        )
        .into());
    }

    config
        .mcp_servers
        .insert(name.to_string(), McpServerDefinition { command, args, env });

    save_config(&config)
}

/// Remove an MCP server from the configuration
pub fn remove_server(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config = load_config()?;

    if !config.mcp_servers.contains_key(name) {
        return Err(format!("Server '{name}' not found").into());
    }

    config.mcp_servers.remove(name);

    save_config(&config)
}

/// Add a tool to auto-approved list
pub fn add_auto_approved_tool(tool_name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config = load_config().unwrap_or_else(|_| AskConfig {
        mcp_servers: HashMap::new(),
        auto_approved_tools: Vec::new(),
        base_url: None,
        model: None,
    });

    if !config.auto_approved_tools.contains(&tool_name.to_string()) {
        config.auto_approved_tools.push(tool_name.to_string());
    }

    save_config(&config)
}

pub fn set_base_url(base_url: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    config.base_url = Some(base_url.to_string());
    save_config(&config)
}

pub fn set_default_model(model: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    config.model = Some(model.to_string());
    save_config(&config)
}

/// Expand environment variables in strings
/// Supports ${VAR} and ${VAR:-default} syntax
fn expand_env_vars(input: &str) -> String {
    let mut result = input.to_string();

    // Match ${VAR:-default} pattern first (more specific)
    let re_with_default = regex::Regex::new(r"\$\{([^:}]+):-([^}]*)\}").unwrap();
    for cap in re_with_default.captures_iter(input) {
        let var_name = &cap[1];
        let default_value = &cap[2];
        let replacement = std::env::var(var_name).unwrap_or_else(|_| default_value.to_string());
        result = result.replace(&cap[0], &replacement);
    }

    // Match ${VAR} pattern
    let re_simple = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    for cap in re_simple.captures_iter(&result.clone()) {
        let var_name = &cap[1];
        if let Ok(value) = std::env::var(var_name) {
            result = result.replace(&cap[0], &value);
        }
    }

    result
}
