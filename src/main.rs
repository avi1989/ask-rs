use crate::commands::Commands;
use crate::commands::mcp_commands::handle_mcp_commands;
use crate::commands::session_commands::handle_session_commands;
use crate::sessions::get_last_session_name;
use clap::Parser;
use crossterm::terminal;

mod commands;
mod config;
mod llms;
mod sessions;
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

    /// Name of the session to use. If provided, this allows you to continue a conversation.
    #[arg(short, long)]
    session: Option<String>,

    /// Enable reply mode. In this mode, the AI will reply to the last question.
    #[arg(short, long)]
    reply: bool,

    /// The OPENAI model to use. Defaults to gpt-4.1-mini or whatever is configured in the config file.
    #[arg(short, long)]
    model: Option<String>,

    /// Question to ask the AI (if no subcommand is provided)
    #[arg(trailing_var_arg = true)]
    question: Vec<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Mcp { command }) => handle_mcp_commands(command),
        Some(Commands::Session { command }) => handle_session_commands(command),
        Some(Commands::Init) => {
            handle_init();
        }
        Some(Commands::SetBaseUrl { url }) => {
            print!("Setting base URL to {}. Continue? [y/N]: ", url);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            let _ = config::set_base_url(&url);
            return;
        }
        Some(Commands::SetDefaultModel { model }) => {
            println!("Settings default model to {model}");
            let _ = config::set_default_model(&model);
            return;
        }
        None => {
            let stdin = get_stdin();
            if cli.question.is_empty() && stdin.is_empty() {
                eprintln!("Error: Please provide a question or use a subcommand (init, mcp)");
                std::process::exit(1);
            } else {
                let model = cli.model;
                let mut question = cli.question.join(" ");
                question = format!("{}\n\n{}", question, stdin);
                let mut session = cli.session;
                if session.is_none() && cli.reply {
                    session = get_last_session_name();
                }

                match llms::ask_question(&question, model, session, cli.verbose).await {
                    Ok(answer) => {
                        // Check if we should use pager for long responses
                        let line_count = answer.lines().count();
                        let (_, height) = terminal::size().unwrap_or((80, 24));

                        if atty::is(atty::Stream::Stdout) && line_count > height as usize {
                            // Render to a Vec<u8> first, then use pager
                            let mut output = Vec::new();
                            markterm::render_text(&answer, None, &mut output, true).unwrap();
                            let rendered = String::from_utf8(output).unwrap();

                            let pager = minus::Pager::new();
                            pager.set_text(&rendered).unwrap();
                            minus::page_all(pager).unwrap();
                        } else {
                            markterm::render_text_to_stdout(
                                &answer,
                                None,
                                markterm::ColorChoice::Auto,
                            )
                            .unwrap();
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }
}

fn get_stdin() -> String {
    use std::io::Read;

    if atty::is(atty::Stream::Stdin) {
        return String::new();
    }

    let mut buffer = String::new();
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    handle
        .read_to_string(&mut buffer)
        .expect("Failed to read from stdin");

    buffer.trim().to_string()
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

    let config_path: std::path::PathBuf = shellexpand::tilde("~/.ask/config")
        .into_owned()
        .parse()
        .unwrap();

    if config_path.exists() {
        eprintln!("Error: ~/.ask/config already exists.");
        eprintln!("Remove it first if you want to reinitialize.");
        std::process::exit(1);
    }

    println!("This will create ~/.ask/config with the following MCP servers:");
    println!();
    println!("  1. filesystem - File system operations (using npx mcp-server-filesystem)");
    println!("     Command: {npx_command} -y mcp-server-filesystem .");
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

    let config = config::AskConfig {
        base_url: None,
        model: None,
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
            println!("✓ Created configuration file at {path:?}");
            println!();
            println!("You can now use ask-rs with MCP tools!");
            println!("Try: ask-rs \"list files in current directory\" or ask-rs mcp list");
        }
        Err(e) => {
            eprintln!("Error creating config: {e}");
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
