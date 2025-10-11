use crate::commands::McpCommands;
use crate::config;

pub fn handle_mcp_commands(command: McpCommands) {
    match command {
        McpCommands::List => {
            handle_list();
        }
        McpCommands::Add {
            name,
            command,
            args,
            env,
        } => {
            handle_add(name, command, args, env);
        }
        McpCommands::Remove { name } => {
            handle_remove(name);
        }
    }
}

fn handle_list() {
    match config::load_config() {
        Ok(cfg) => {
            if cfg.mcp_servers.is_empty() {
                println!("No MCP servers configured.");
                println!("Add one with: ask-rs mcp add <name> <command> --args <args>");
                return;
            }

            println!("Configured MCP servers:\n");
            for (name, server) in &cfg.mcp_servers {
                println!("  {name}");
                println!("    Command: {}", server.command);
                if !server.args.is_empty() {
                    println!("    Args: {}", server.args.join(" "));
                }
                if !server.env.is_empty() {
                    println!("    Env:");
                    for (k, v) in &server.env {
                        println!("      {k}={v}");
                    }
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("Error loading config: {e}");
            eprintln!("Run 'ask-rs mcp add' to create your first MCP server.");
        }
    }
}

fn handle_add(name: String, command: String, args: Vec<String>, env_pairs: Vec<String>) {
    let mut env = std::collections::HashMap::new();
    for pair in env_pairs {
        if let Some((key, value)) = pair.split_once('=') {
            env.insert(key.to_string(), value.to_string());
        } else {
            eprintln!("Warning: Invalid env format '{pair}', expected KEY=VALUE");
        }
    }

    match config::add_server(&name, command, args, env) {
        Ok(path) => {
            println!("✓ Added MCP server '{name}' to {path:?}");
        }
        Err(e) => {
            eprintln!("Error adding server: {e}");
            std::process::exit(1);
        }
    }
}

fn handle_remove(name: String) {
    match config::remove_server(&name) {
        Ok(path) => {
            println!("✓ Removed MCP server '{name}' from {path:?}");
        }
        Err(e) => {
            eprintln!("Error removing server: {e}");
            std::process::exit(1);
        }
    }
}
