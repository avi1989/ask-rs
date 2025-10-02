# ask-rs

A powerful AI assistant CLI tool with dynamic MCP (Model Context Protocol) server support. Ask questions and let AI interact with your development tools through configurable MCP servers.

## Features

- 🤖 **AI-Powered Assistant** - Ask questions and get intelligent responses powered by OpenAI
- 🔌 **Dynamic MCP Integration** - Connect to any MCP server using Claude Code's configuration format
- 🛠️ **Built-in Tools** - File system operations, command execution, and more
- 🔒 **Permission-Based Execution** - User confirmation required before executing MCP tools and commands
- ⚙️ **Easy Configuration** - Manage MCP servers via CLI commands
- 🌍 **Environment Variables** - Support for `${VAR}` and `${VAR:-default}` expansion
- 📦 **Zero Hardcoding** - Add new MCP servers without touching code

## Installation

### Prerequisites

- Rust 1.70 or higher
- OpenAI API key
- MCP servers (e.g., `uvx` for Python-based MCP servers)

### Build from Source

```bash
git clone https://github.com/yourusername/ask-rs.git
cd ask-rs
cargo build --release
```

### Set up Environment

```bash
# Required: OpenAI API key
export OPENAI_API_KEY="your-api-key-here"

# Optional: Custom model (defaults to gpt-4.1)
export OPENAI_MODEL="gpt-4"
```

## Quick Start

### 1. Ask a Question

```bash
cargo run -- what files are in the current directory
```

### 2. Add MCP Servers

```bash
# Add git MCP server
cargo run -- add git uvx --args mcp-server-git

# Add filesystem server with custom path
cargo run -- add filesystem uvx \
  --args "mcp-server-filesystem,--allowed-directory,${HOME}/projects"
```

### 3. List Configured Servers

```bash
cargo run -- list
```

Output:
```
Configured MCP servers:

  git
    Command: uvx
    Args: mcp-server-git

  filesystem
    Command: uvx
    Args: mcp-server-filesystem --allowed-directory ${HOME}/projects
```

### 4. Ask Questions Using MCP Tools

```bash
# Git operations
cargo run -- show me the pending git changes

# Filesystem operations
cargo run -- list all TypeScript files in my projects directory
```

## Configuration

### Configuration File

MCP servers are configured in `~/.askrc` using Claude Code's `.mcp.json` format:

```json
{
  "mcpServers": {
    "git": {
      "command": "uvx",
      "args": ["mcp-server-git"],
      "env": {}
    },
    "filesystem": {
      "command": "uvx",
      "args": ["mcp-server-filesystem", "--path", "${HOME}/projects"],
      "env": {
        "DEBUG": "1"
      }
    },
    "weather": {
      "command": "node",
      "args": ["/path/to/weather-server/index.js"],
      "env": {
        "WEATHER_API_KEY": "${WEATHER_API_KEY:-default-key}"
      }
    }
  },
  "autoApprovedTools": [
    "git_status",
    "git_log",
    "git_diff"
  ]
}
```

The `autoApprovedTools` array contains tools that will execute without prompting.

### Environment Variable Expansion

Supports two formats:
- `${VAR}` - Expands to the value of `VAR`
- `${VAR:-default}` - Uses `default` if `VAR` is not set

## CLI Commands

### Ask Questions (Default)

```bash
ask-rs <question>
```

**Examples:**
```bash
ask-rs what is the current git branch
ask-rs explain the code in main.rs
ask-rs list all TODO comments in the project
```

### Manage MCP Servers

#### List Servers

```bash
ask-rs list
```

Shows all configured MCP servers with their settings.

#### Add Server

```bash
ask-rs add <name> <command> [OPTIONS]
```

**Options:**
- `-a, --args <ARGS>` - Command arguments (comma-separated)
- `-e, --env <ENV>` - Environment variables in `KEY=VALUE` format (comma-separated)

**Examples:**
```bash
# Simple server
ask-rs add git uvx --args mcp-server-git

# Server with multiple arguments
ask-rs add db uvx --args "mcp-server-postgres,--host,localhost,--port,5432"

# Server with environment variables
ask-rs add api node \
  --args "/path/to/server.js" \
  --env "API_KEY=secret,LOG_LEVEL=debug"
```

#### Remove Server

```bash
ask-rs remove <name>
```

**Example:**
```bash
ask-rs remove weather
```

### Manage Auto-Approvals

#### List Auto-Approved Tools

```bash
ask-rs approvals
```

Shows all tools that will execute without prompting.

#### Approve a Tool

```bash
ask-rs approve <tool_name>
```

**Example:**
```bash
# Approve git_status to auto-execute
ask-rs approve git_status

# Approve execute_command to auto-execute all shell commands
ask-rs approve execute_command
```

#### Unapprove a Tool

```bash
ask-rs unapprove <tool_name>
```

**Example:**
```bash
ask-rs unapprove git_status
```

## Built-in Tools

The following tools are available by default:

### File System Tools

- **`list_all_files`** - List files in a directory
- **`read_file`** - Read file contents

### Command Execution

- **`execute_command`** - Execute shell commands (requires user confirmation)

## Security & Permissions

For safety, the application asks for user confirmation before executing:

1. **Shell Commands** - Any command execution via `execute_command` tool
2. **MCP Tools** - All MCP tool calls from configured servers

### Permission Prompt Example

When an MCP tool is about to be executed, you'll see:

```
MCP Tool: git_status
Arguments:
{
  "repo_path": "."
}

Execute MCP tool 'git_status'? [y/N/A]:
```

**Response Options:**
- `y` or `yes` - Approve this single execution
- `n` or `N` or Enter - Cancel execution
- `a` or `all` - **Auto-approve all future calls to this tool for the session**

### Auto-Approval

When you respond with `A` (or `all`), all future invocations of that specific tool will be automatically approved without prompting. This is useful when:

- You trust a particular MCP tool completely
- The AI needs to call the same tool multiple times
- You want to streamline repetitive operations

**Example:**
```
Execute MCP tool 'git_status'? [y/N/A]: A
All future 'git_status' calls will be auto-approved for this session.
```

Subsequent calls to `git_status` will show:
```
MCP Tool: git_status
Arguments:
{
  "repo_path": "."
}
[Auto-approved]
```

**Note:** Auto-approvals are **persisted to `~/.askrc`** and will be remembered across sessions.

### Managing Auto-Approvals via CLI

You can also manage auto-approved tools using CLI commands:

```bash
# Approve a tool
ask-rs approve git_status

# List all approved tools
ask-rs approvals

# Remove approval
ask-rs unapprove git_status
```

This ensures you have full control over what actions the AI performs on your system while maintaining convenience for trusted tools.

### MCP Tools

All tools from configured MCP servers are automatically loaded with the server name as a prefix.

For example, a git MCP server provides:
- `git_status` - Get repository status
- `git_diff` - Show file differences
- `git_log` - View commit history
- `git_add` - Stage files
- And more...

## Available MCP Servers

### Official MCP Servers

Install via `uvx`:

```bash
# Git operations
ask-rs add git uvx --args mcp-server-git

# Filesystem access
ask-rs add filesystem uvx --args mcp-server-filesystem

# PostgreSQL database
ask-rs add postgres uvx --args mcp-server-postgres

# GitHub API
ask-rs add github uvx --args mcp-server-github
```

### Custom MCP Servers

You can add any MCP server that follows the protocol:

```bash
ask-rs add myserver node --args "/path/to/my/server.js"
ask-rs add myserver python --args "/path/to/my/server.py"
```

## Examples

### Git Workflow

```bash
# Check status
ask-rs what are my pending git changes

# Create commit message
ask-rs generate a commit message for my staged changes

# View history
ask-rs show me the last 5 commits
```

### Code Analysis

```bash
# Understand code
ask-rs explain what main.rs does

# Find patterns
ask-rs find all functions that use async/await

# Generate documentation
ask-rs write documentation for the config module
```

### Project Management

```bash
# Find TODOs
ask-rs list all TODO comments with their file locations

# Analyze structure
ask-rs describe the project structure

# Dependencies
ask-rs what dependencies does this project use
```

## Configuration Files

### File Locations

Configuration is loaded from (in order):
1. `~/.askrc` (preferred)
2. `./.askrc` (project-specific)

### Example Configuration

See `.askrc.example` for a complete configuration example.

## Development

### Project Structure

```
ask-rs/
├── src/
│   ├── main.rs          # CLI entry point and command handlers
│   ├── llms.rs          # OpenAI integration and tool execution
│   ├── config.rs        # Configuration loading and management
│   ├── shell.rs         # Shell detection utilities
│   └── tools/
│       ├── mod.rs       # Built-in tools (file ops, commands)
│       └── mcp.rs       # Generic MCP server integration
├── Cargo.toml
├── README.md
└── .askrc.example
```

### Adding Built-in Tools

Edit `src/tools/mod.rs` to add new built-in tools that don't require MCP servers.

### Testing

```bash
# Build
cargo build

# Run tests
cargo test

# Check
cargo check
```

## Troubleshooting

### "No configuration file found"

Create `~/.askrc` or use the `add` command:

```bash
ask-rs add git uvx --args mcp-server-git
```

### "Failed to connect to MCP server"

Verify:
1. MCP server is installed (`uvx` or the specified command)
2. Command and arguments are correct
3. Required environment variables are set

### "OPENAI_API_KEY is not set"

Set your OpenAI API key:

```bash
export OPENAI_API_KEY="sk-..."
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key (required) | - |
| `OPENAI_MODEL` | Model to use | `gpt-4.1` |

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

[Your chosen license]

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Uses [OpenAI API](https://openai.com/api/)
- Integrates with [MCP (Model Context Protocol)](https://modelcontextprotocol.io/)
- Inspired by [Claude Code](https://claude.com/claude-code)

## Related Projects

- [MCP Servers](https://github.com/modelcontextprotocol/servers) - Official MCP server implementations
- [Claude Code](https://claude.com/claude-code) - Anthropic's AI coding assistant
- [rmcp](https://github.com/modelcontextprotocol/rust-sdk) - Rust MCP SDK
