use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
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
pub enum McpCommands {
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

#[derive(Subcommand)]
pub enum SessionCommands {
    /// List all sessions
    List,

    /// Shows the conversation for a session
    Show { name: Option<String> },

    /// Saves the last chat as a named session
    Save { name: String },
}
