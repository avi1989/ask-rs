use clap::{Parser, Subcommand};

mod config;
mod llms;
mod tools;
mod shell;

#[derive(Parser)]
#[command(name = "ask-rs")]
#[command(about = "AI assistant with MCP tool support", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

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
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::List) => {
            handle_list();
        }
        Some(Commands::Add { name, command, args, env }) => {
            handle_add(name, command, args, env);
        }
        Some(Commands::Remove { name }) => {
            handle_remove(name);
        }
        None => {
            // Default behavior: ask a question
            if cli.question.is_empty() {
                eprintln!("Error: Please provide a question or use a subcommand (list, add, remove)");
                std::process::exit(1);
            }
            let question = cli.question.join(" ");
            let answer = llms::ask_question(&question).await.unwrap();
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
