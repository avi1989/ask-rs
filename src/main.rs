use clap::{Parser, Subcommand};

mod config;
mod llms;
mod shell;
mod tools;

#[derive(Parser)]
#[command(name = "ask-rs")]
#[command(about = "AI assistant with MCP tool support", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// The OPENAI model to use. Defaults to gpt-4.1-mini.
    #[arg(short, long)]
    model: Option<String>,

    /// Question to ask the AI (if no subcommand is provided)
    #[arg(trailing_var_arg = true)]
    question: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// List configured MCP servers
    List,

    /// Add a new MCP server
    Add {
        /// Name of the MCP server (used as tool prefix)
        name: String,

        /// Command to execute (e.g., "uvx", "node")
        command: String,

        /// Arguments for the command
        #[arg(short, long, value_delimiter = ',')]
        args: Vec<String>,

        /// Environment variables in KEY=VALUE format
        #[arg(short, long, value_delimiter = ',')]
        env: Vec<String>,
    },

    /// Remove an MCP server
    Remove {
        /// Name of the MCP server to remove
        name: String,
    },

    /// Approve a tool to auto-execute without prompting
    Approve {
        /// Name of the tool to auto-approve (e.g., "git_status", "execute_command")
        tool_name: String,
    },

    /// Unapprove a tool (require prompting again)
    Unapprove {
        /// Name of the tool to remove from auto-approve list
        tool_name: String,
    },

    /// List all auto-approved tools
    Approvals,

    /// Initialize ~/.askrc with default MCP servers
    Init,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List) => {
            handle_list();
        }
        Some(Commands::Add {
            name,
            command,
            args,
            env,
        }) => {
            handle_add(name, command, args, env);
        }
        Some(Commands::Remove { name }) => {
            handle_remove(name);
        }
        Some(Commands::Approve { tool_name }) => {
            handle_approve(tool_name);
        }
        Some(Commands::Unapprove { tool_name }) => {
            handle_unapprove(tool_name);
        }
        Some(Commands::Approvals) => {
            handle_list_approvals();
        }
        Some(Commands::Init) => {
            handle_init();
        }
        None => {
            if cli.question.is_empty() {
                eprintln!(
                    "Error: Please provide a question or use a subcommand (list, add, remove)"
                );
                std::process::exit(1);
            }
            let model = cli.model.unwrap_or("gpt-4.1-mini".to_string());
            let question = cli.question.join(" ");
            let answer = llms::ask_question(&question, &model, cli.verbose)
                .await
                .unwrap();
            markterm::render_text_to_stdout(&answer, None, markterm::ColorChoice::Auto).unwrap();
        }
    }
}

fn handle_list() {
    match config::load_config() {
        Ok(cfg) => {
            if cfg.mcp_servers.is_empty() {
                println!("No MCP servers configured.");
                println!("Add one with: ask-rs add <name> <command> --args <args>");
                return;
            }

            println!("Configured MCP servers:\n");
            for (name, server) in &cfg.mcp_servers {
                println!("  {}", name);
                println!("    Command: {}", server.command);
                if !server.args.is_empty() {
                    println!("    Args: {}", server.args.join(" "));
                }
                if !server.env.is_empty() {
                    println!("    Env:");
                    for (k, v) in &server.env {
                        println!("      {}={}", k, v);
                    }
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            eprintln!("Run 'ask-rs add' to create your first MCP server.");
        }
    }
}

fn handle_add(name: String, command: String, args: Vec<String>, env_pairs: Vec<String>) {
    let mut env = std::collections::HashMap::new();
    for pair in env_pairs {
        if let Some((key, value)) = pair.split_once('=') {
            env.insert(key.to_string(), value.to_string());
        } else {
            eprintln!("Warning: Invalid env format '{}', expected KEY=VALUE", pair);
        }
    }

    match config::add_server(&name, command, args, env) {
        Ok(path) => {
            println!("✓ Added MCP server '{}' to {:?}", name, path);
        }
        Err(e) => {
            eprintln!("Error adding server: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_remove(name: String) {
    match config::remove_server(&name) {
        Ok(path) => {
            println!("✓ Removed MCP server '{}' from {:?}", name, path);
        }
        Err(e) => {
            eprintln!("Error removing server: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_approve(tool_name: String) {
    match config::add_auto_approved_tool(&tool_name) {
        Ok(path) => {
            println!(
                "✓ Tool '{}' will be auto-approved (saved to {:?})",
                tool_name, path
            );
            println!("  This tool will execute without prompting in future sessions.");
        }
        Err(e) => {
            eprintln!("Error approving tool: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_unapprove(tool_name: String) {
    match config::remove_auto_approved_tool(&tool_name) {
        Ok(path) => {
            println!(
                "✓ Tool '{}' removed from auto-approve list (saved to {:?})",
                tool_name, path
            );
            println!("  This tool will require confirmation before executing.");
        }
        Err(e) => {
            eprintln!("Error unapproving tool: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_list_approvals() {
    match config::list_auto_approved_tools() {
        Ok(tools) => {
            if tools.is_empty() {
                println!("No auto-approved tools.");
                println!("Add one with: ask-rs approve <tool_name>");
                return;
            }

            println!("Auto-approved tools:\n");
            for tool in &tools {
                println!("  ✓ {}", tool);
            }
            println!("\nThese tools will execute without prompting.");
            println!("Remove with: ask-rs unapprove <tool_name>");
        }
        Err(e) => {
            eprintln!("Error listing approvals: {}", e);
            eprintln!("No configuration file found.");
        }
    }
}

fn handle_init() {
    let npx_command = if cfg!(target_os = "windows") {
        if check_command_exists("npx.cmd") {
            "npx.cmd"
        } else if check_command_exists("npx") {
            "npx"
        } else {
            eprintln!("Error: 'npx' command not found in PATH.");
            eprintln!("Please install Node.js/npm to use the filesystem MCP server.");
            std::process::exit(1);
        }
    } else {
        if !check_command_exists("npx") {
            eprintln!("Error: 'npx' command not found in PATH.");
            eprintln!("Please install Node.js/npm to use the filesystem MCP server.");
            std::process::exit(1);
        }
        "npx"
    };

    let uvx_exists = check_command_exists("uvx");
    if !uvx_exists {
        eprintln!("Error: 'uvx' command not found in PATH.");
        eprintln!("Please install uv (https://docs.astral.sh/uv/) to use the git MCP server.");
        std::process::exit(1);
    }

    let config_path: std::path::PathBuf =
        shellexpand::tilde("~/.askrc").into_owned().parse().unwrap();
    if config_path.exists() {
        eprintln!("Error: ~/.askrc already exists.");
        eprintln!("Remove it first if you want to reinitialize.");
        std::process::exit(1);
    }

    println!("This will create ~/.askrc with the following MCP servers:");
    println!();
    println!("  1. filesystem - File system operations (using npx mcp-server-filesystem)");
    println!("     Command: {} -y mcp-server-filesystem .", npx_command);
    println!("     Provides tools for reading, writing, and managing files");
    println!();
    println!("  2. git - Git repository operations (using uvx mcp-server-git)");
    println!("     Command: uvx mcp-server-git");
    println!("     Provides tools for git commands and repository management");
    println!();
    println!("Location: {}", config_path.display());
    println!();

    print!("Continue? [y/N]: ");
    use std::io::Write;
    std::io::stdout().flush().unwrap();

    let mut response = String::new();
    std::io::stdin().read_line(&mut response).unwrap();
    let response = response.trim().to_lowercase();

    if response != "y" && response != "yes" {
        println!("Cancelled.");
        return;
    }

    let config = config::AskRcConfig {
        mcp_servers: {
            let mut servers = std::collections::HashMap::new();
            servers.insert(
                "filesystem".to_string(),
                config::McpServerDefinition {
                    command: npx_command.to_string(),
                    args: vec![
                        "-y".to_string(),
                        "mcp-server-filesystem".to_string(),
                        ".".to_string(),
                    ],
                    env: {
                        let mut env = std::collections::HashMap::new();
                        env.insert("DEBUG".to_string(), "1".to_string());
                        env
                    },
                },
            );
            servers.insert(
                "git".to_string(),
                config::McpServerDefinition {
                    command: "uvx".to_string(),
                    args: vec!["mcp-server-git".to_string()],
                    env: std::collections::HashMap::new(),
                },
            );
            servers
        },
        auto_approved_tools: Vec::new(),
    };

    match config::save_config(&config) {
        Ok(path) => {
            println!("✓ Created configuration file at {:?}", path);
            println!();
            println!("You can now use ask-rs with MCP tools!");
            println!("Try: ask-rs list files in current directory");
        }
        Err(e) => {
            eprintln!("Error creating config: {}", e);
            std::process::exit(1);
        }
    }
}

fn check_command_exists(command: &str) -> bool {
    let checker = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };

    std::process::Command::new(checker)
        .arg(command)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
