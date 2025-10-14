use crate::tools::mcp::McpServerConfig;
use anyhow::{Context, Result};
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

    #[serde(rename = "modelAliases", default)]
    pub model_aliases: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpServerDefinition {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

pub fn load_config() -> Result<AskConfig> {
    let config_path = find_config_file()?;
    let contents = fs::read_to_string(&config_path)
        .context(format!("Failed to read config file at {:?}", config_path))?;

    let config: AskConfig = serde_json::from_str(&contents).context(format!(
        "Failed to parse config file at {:?}. Check JSON syntax.",
        config_path
    ))?;

    Ok(config)
}

fn find_config_file() -> Result<PathBuf> {
    let home_config: PathBuf = shellexpand::tilde("~/.ask/config")
        .into_owned()
        .parse()
        .context("Failed to parse config file path")?;

    if home_config.exists() {
        return Ok(home_config);
    }

    anyhow::bail!("No configuration file found. Create ~/.ask/config or run 'ask init'")
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

pub fn save_config(config: &AskConfig) -> Result<PathBuf> {
    let config_path: PathBuf = shellexpand::tilde("~/.ask/config")
        .into_owned()
        .parse()
        .context("Failed to parse config file path")?;

    if let Some(config_dir) = config_path.parent()
        && !config_dir.exists()
    {
        fs::create_dir_all(config_dir).context(format!(
            "Failed to create config directory at {:?}",
            config_dir
        ))?;
    }

    let json =
        serde_json::to_string_pretty(config).context("Failed to serialize config to JSON")?;

    fs::write(&config_path, json)
        .context(format!("Failed to write config to {:?}", config_path))?;

    Ok(config_path)
}

pub fn add_server(
    name: &str,
    command: String,
    args: Vec<String>,
    env: HashMap<String, String>,
) -> Result<PathBuf> {
    let mut config = load_config().unwrap_or_else(|_| AskConfig {
        mcp_servers: HashMap::new(),
        auto_approved_tools: Vec::new(),
        base_url: None,
        model: None,
        model_aliases: HashMap::new(),
    });

    if config.mcp_servers.contains_key(name) {
        anyhow::bail!(
            "Server '{}' already exists. Remove it first with: ask mcp remove {}",
            name,
            name
        );
    }

    config
        .mcp_servers
        .insert(name.to_string(), McpServerDefinition { command, args, env });

    save_config(&config).context(format!(
        "Failed to save config after adding server '{}'",
        name
    ))
}

pub fn remove_server(name: &str) -> Result<PathBuf> {
    let mut config = load_config().context("Failed to load config to remove server")?;

    if !config.mcp_servers.contains_key(name) {
        anyhow::bail!("Server '{}' not found in configuration", name);
    }

    config.mcp_servers.remove(name);

    save_config(&config).context(format!(
        "Failed to save config after removing server '{}'",
        name
    ))
}

pub fn add_auto_approved_tool(tool_name: &str) -> Result<PathBuf> {
    let mut config = load_config().unwrap_or_else(|_| AskConfig {
        mcp_servers: HashMap::new(),
        auto_approved_tools: Vec::new(),
        base_url: None,
        model: None,
        model_aliases: HashMap::new(),
    });

    if !config.auto_approved_tools.contains(&tool_name.to_string()) {
        config.auto_approved_tools.push(tool_name.to_string());
    }

    save_config(&config).context(format!(
        "Failed to save config after adding auto-approved tool '{}'",
        tool_name
    ))
}

pub fn set_base_url(base_url: &str) -> Result<PathBuf> {
    let mut config = load_config().context("Failed to load config to set base URL")?;

    config.base_url = Some(base_url.to_string());

    save_config(&config).context("Failed to save config after setting base URL")
}

pub fn set_default_model(model: &str) -> Result<PathBuf> {
    let mut config = load_config().context("Failed to load config to set default model")?;

    config.model = Some(model.to_string());

    save_config(&config).context("Failed to save config after setting default model")
}

/// Expand environment variables in strings
/// Supports ${VAR} and ${VAR:-default} syntax
fn expand_env_vars(input: &str) -> String {
    use once_cell::sync::Lazy;

    // Compile regexes once and reuse
    static RE_WITH_DEFAULT: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"\$\{([^:}]+):-([^}]*)\}").expect("Failed to compile regex")
    });
    static RE_SIMPLE: Lazy<regex::Regex> =
        Lazy::new(|| regex::Regex::new(r"\$\{([^}]+)\}").expect("Failed to compile regex"));

    let mut result = input.to_string();

    // Match ${VAR:-default} pattern first (more specific)
    for cap in RE_WITH_DEFAULT.captures_iter(input) {
        let var_name = &cap[1];
        let default_value = &cap[2];
        let replacement = std::env::var(var_name).unwrap_or_else(|_| default_value.to_string());
        result = result.replace(&cap[0], &replacement);
    }

    // Match ${VAR} pattern
    let temp_result = result.clone();
    for cap in RE_SIMPLE.captures_iter(&temp_result) {
        let var_name = &cap[1];
        if let Ok(value) = std::env::var(var_name) {
            result = result.replace(&cap[0], &value);
        }
    }

    result
}
