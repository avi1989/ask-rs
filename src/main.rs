use crate::sessions::{get_all_sessions, get_last_session_name, get_session};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestUserMessageContent,
};
use clap::{Parser, Subcommand};
use crossterm::terminal;

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

#[derive(Subcommand)]
enum Commands {
    /// MCP server and tool management
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Initialize ~/.ask/config with default MCP servers
    Init,

    /// Set the OpenAI compatible URL for the LLM
    SetBaseUrl { url: String },

    /// Set the default model to use for the LLM.
    SetDefaultModel { model: String },
}

#[derive(Subcommand)]
enum McpCommands {
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
}

#[derive(Subcommand)]
enum SessionCommands {
    /// List all sessions
    List,

    /// Shows the conversation for a session
    Show { name: Option<String> },

    /// Saves the last chat as a named session
    Save { name: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Mcp { command }) => match command {
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
            McpCommands::Approve { tool_name } => {
                handle_approve(tool_name);
            }
            McpCommands::Unapprove { tool_name } => {
                handle_unapprove(tool_name);
            }
            McpCommands::Approvals => {
                handle_list_approvals();
            }
        },
        Some(Commands::Session { command }) => match command {
            SessionCommands::List => {
                let sessions = get_all_sessions();
                for session in sessions {
                    println!("{:<20} {}", session.name, session.created);
                }
            }
            SessionCommands::Show { name } => {
                let name =
                    name.unwrap_or_else(|| get_last_session_name().unwrap_or("last".to_string()));
                handle_show_session(name);
            }
            SessionCommands::Save { name } => {
                match get_session("last") {
                    Some(session) => {
                        let _ = sessions::save_session(&name, &session, None);
                        println!("Saved session as {name}");
                    }
                    None => {
                        eprintln!("Error: No session to save");
                        std::process::exit(1);
                    }
                }
            }
        },
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
        },
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

struct MessageBoxConfig {
    label: &'static str,
    color: &'static str,
    max_width_percent: f32,
    align_right: bool,
    left_margin: usize,
}

fn render_message_box(
    output: &mut String,
    text: &str,
    terminal_width: u16,
    config: MessageBoxConfig,
    use_colors: bool,
) {
    use std::fmt::Write as FmtWrite;

    let max_box_width = (terminal_width as f32 * config.max_width_percent) as usize;
    let box_padding = 3;

    let lines: Vec<&str> = text.lines().collect();
    let content_width = lines
        .iter()
        .map(|line| line.len())
        .max()
        .unwrap_or(0)
        .min(max_box_width - box_padding * 2);

    let box_width = content_width + box_padding * 2;
    let left_margin = if config.align_right {
        terminal_width.saturating_sub(box_width as u16 + 2) as usize
    } else {
        config.left_margin
    };

    let label_indent = if config.align_right {
        left_margin + box_width - config.label.len()
    } else {
        left_margin
    };

    if use_colors {
        write!(output, "{}", " ".repeat(label_indent)).unwrap();
        writeln!(output, "{}{}\x1b[0m", config.color, config.label).unwrap();

        // Top border
        write!(output, "{}", " ".repeat(left_margin)).unwrap();
        writeln!(output, "{}╭{}╮\x1b[0m", config.color, "─".repeat(box_width)).unwrap();

        // Content
        for line in lines {
            let display_line = if line.len() > content_width {
                &line[..content_width]
            } else {
                line
            };
            let padding = content_width - display_line.len();

            write!(output, "{}", " ".repeat(left_margin)).unwrap();
            write!(output, "{}│\x1b[0m", config.color).unwrap();
            write!(
                output,
                "{}{}{}",
                " ".repeat(box_padding),
                display_line,
                " ".repeat(padding + box_padding)
            )
            .unwrap();
            writeln!(output, "{}│\x1b[0m", config.color).unwrap();
        }

        // Bottom border
        write!(output, "{}", " ".repeat(left_margin)).unwrap();
        writeln!(output, "{}╰{}╯\x1b[0m", config.color, "─".repeat(box_width)).unwrap();
    } else {
        // Simple text output without colors and box drawing
        writeln!(output, "{}", config.label).unwrap();
        writeln!(output, "{}", "-".repeat(config.label.len())).unwrap();
        for line in lines {
            writeln!(output, "{}", line).unwrap();
        }
    }
    writeln!(output).unwrap();
}

fn handle_show_session(name: String) {
    use std::fmt::Write as FmtWrite;

    let session = get_session(&name);
    match session {
        Some(session) => {
            let is_interactive = atty::is(atty::Stream::Stdout);
            let (width, _) = terminal::size().unwrap_or((80, 24));
            let mut output = String::new();

            writeln!(&mut output).unwrap();

            // Display session name header (centered in interactive mode)
            if is_interactive {
                let header_text = format!("═══ Session: {} ═══", name);
                let header_len = header_text.chars().count();
                let left_padding = if header_len < width as usize {
                    (width as usize - header_len) / 2
                } else {
                    0
                };
                writeln!(
                    &mut output,
                    "{}\x1b[1m\x1b[35m{}\x1b[0m",
                    " ".repeat(left_padding),
                    header_text
                )
                .unwrap();
            } else {
                writeln!(&mut output, "=== Session: {} ===", name).unwrap();
            }
            writeln!(&mut output).unwrap();

            for message in session {
                match message {
                    ChatCompletionRequestMessage::User(message) => {
                        if let ChatCompletionRequestUserMessageContent::Text(text) = message.content
                        {
                            render_message_box(
                                &mut output,
                                &text,
                                width,
                                MessageBoxConfig {
                                    label: "User",
                                    color: "\x1b[36m",
                                    max_width_percent: 0.6,
                                    align_right: true,
                                    left_margin: 0,
                                },
                                is_interactive,
                            );
                        }
                    }
                    ChatCompletionRequestMessage::Assistant(message) => {
                        if let Some(content) = &message.content
                            && let ChatCompletionRequestAssistantMessageContent::Text(text) =
                                content
                        {
                            render_message_box(
                                &mut output,
                                text,
                                width,
                                MessageBoxConfig {
                                    label: "Assistant",
                                    color: "\x1b[32m",
                                    max_width_percent: 0.8,
                                    align_right: false,
                                    left_margin: 2,
                                },
                                is_interactive,
                            );
                        }
                    }
                    _ => {}
                }
            }

            if is_interactive {
                let pager = minus::Pager::new();
                pager.set_text(&output).unwrap();
                minus::page_all(pager).unwrap();
            } else {
                print!("{}", output);
            }
        }
        None => {
            println!("Session not found");
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

fn handle_approve(tool_name: String) {
    match config::add_auto_approved_tool(&tool_name) {
        Ok(path) => {
            println!("✓ Tool '{tool_name}' will be auto-approved (saved to {path:?})");
            println!("  This tool will execute without prompting in future sessions.");
        }
        Err(e) => {
            eprintln!("Error approving tool: {e}");
            std::process::exit(1);
        }
    }
}

fn handle_unapprove(tool_name: String) {
    match config::remove_auto_approved_tool(&tool_name) {
        Ok(path) => {
            println!("✓ Tool '{tool_name}' removed from auto-approve list (saved to {path:?})");
            println!("  This tool will require confirmation before executing.");
        }
        Err(e) => {
            eprintln!("Error unapproving tool: {e}");
            std::process::exit(1);
        }
    }
}

fn handle_list_approvals() {
    match config::list_auto_approved_tools() {
        Ok(tools) => {
            if tools.is_empty() {
                println!("No auto-approved tools.");
                println!("Add one with: ask-rs mcp approve <tool_name>");
                return;
            }

            println!("Auto-approved tools:\n");
            for tool in &tools {
                println!("  ✓ {tool}");
            }
            println!("\nThese tools will execute without prompting.");
            println!("Remove with: ask-rs mcp unapprove <tool_name>");
        }
        Err(e) => {
            eprintln!("Error listing approvals: {e}");
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
