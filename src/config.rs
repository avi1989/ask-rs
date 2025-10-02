//! Configuration file loading for MCP servers
//!
//! Loads MCP server configurations from `~/.askrc` using Claude Code's `.mcp.json` format.

use crate::tools::mcp::McpServerConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Top-level configuration file structure
#[derive(Debug, Deserialize, Serialize)]
pub struct AskRcConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerDefinition>,
}

/// Individual MCP server configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpServerDefinition {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Load configuration from ~/.askrc or ./.askrc
pub fn load_config() -> Result<AskRcConfig, Box<dyn std::error::Error>> {
    let config_path = find_config_file()?;
    let contents = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config file {:?}: {}", config_path, e))?;

    let config: AskRcConfig = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse config file {:?}: {}", config_path, e))?;

    Ok(config)
}

/// Find the configuration file, checking ~/.askrc first, then ./.askrc
fn find_config_file() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Try ~/.askrc first
    let home_config: PathBuf = shellexpand::tilde("~/.askrc").into_owned().parse()?;
    if home_config.exists() {
        return Ok(home_config);
    }

    // Try ./.askrc
    let local_config = PathBuf::from("./.askrc");
    if local_config.exists() {
        return Ok(local_config);
    }

    Err("No configuration file found. Create ~/.askrc or ./.askrc".into())
}

/// Convert configuration to MCP server configs with tool prefixes
pub fn config_to_servers(config: &AskRcConfig) -> Vec<(String, McpServerConfig)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env_vars() {
        std::env::set_var("TEST_VAR", "test_value");

        assert_eq!(expand_env_vars("${TEST_VAR}"), "test_value");
        assert_eq!(expand_env_vars("prefix_${TEST_VAR}_suffix"), "prefix_test_value_suffix");
        assert_eq!(expand_env_vars("${NONEXISTENT:-default}"), "default");
        assert_eq!(expand_env_vars("${TEST_VAR:-default}"), "test_value");
    }
}
