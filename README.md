# ask

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

### Download Binary

**Linux (amd64):**
```bash
curl -L https://github.com/avi1989/ask-rs/releases/latest/download/ask_amd64 -o ask
chmod +x ask
sudo mv ask /usr/local/bin/
```

**macOS:**
```bash
curl -L https://github.com/avi1989/ask-rs/releases/latest/download/ask_darwin -o ask
chmod +x ask
sudo mv ask /usr/local/bin/
```

**Windows:**
Download `ask.exe` from [releases](https://github.com/avi1989/ask-rs/releases/latest)

### Build from Source

```bash
git clone https://github.com/avi1989/ask-rs.git
cd ask-rs
cargo build --release
```

### Set up Environment

```bash
# Required: OpenAI API key
export OPENAI_API_KEY="your-api-key-here"
```

## Quick Start

### 1. Ask a Question

```bash
ask  what files are in the current directory
```

### 2. Add MCP Servers

```bash
# Add git MCP server
ask  add git uvx --args mcp-server-git

# Add filesystem server with custom path
ask  add filesystem uvx \
  --args "mcp-server-filesystem,--allowed-directory,${HOME}/projects"
```

### 3. List Configured Servers

```bash
ask list
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
ask show me the pending git changes

# Filesystem operations
ask list all TypeScript files in my projects directory
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
ask <question>
```

**Examples:**
```bash
ask what is the current git branch
ask explain the code in main.rs
ask list all TODO comments in the project
```

### Manage MCP Servers

#### List Servers

```bash
ask list
```

Shows all configured MCP servers with their settings.

#### Add Server

```bash
ask add <name> <command> [OPTIONS]
```

**Options:**
- `-a, --args <ARGS>` - Command arguments (comma-separated)
- `-e, --env <ENV>` - Environment variables in `KEY=VALUE` format (comma-separated)

**Examples:**
```bash
# Simple server
ask add git uvx --args mcp-server-git

# Server with multiple arguments
ask add db uvx --args "mcp-server-postgres,--host,localhost,--port,5432"

# Server with environment variables
ask add api node \
  --args "/path/to/server.js" \
  --env "API_KEY=secret,LOG_LEVEL=debug"
```

#### Remove Server

```bash
ask remove <name>
```

**Example:**
```bash
ask remove weather
```

### Manage Auto-Approvals

#### List Auto-Approved Tools

```bash
ask approvals
```

Shows all tools that will execute without prompting.

#### Approve a Tool

```bash
ask approve <tool_name>
```

**Example:**
```bash
# Approve git_status to auto-execute
ask approve git_status

# Approve execute_command to auto-execute all shell commands
ask approve execute_command
```

#### Unapprove a Tool

```bash
ask unapprove <tool_name>
```

**Example:**
```bash
ask unapprove git_status
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
ask approve git_status

# List all approved tools
ask approvals

# Remove approval
ask unapprove git_status
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
ask add git uvx --args mcp-server-git

# Filesystem access
ask add filesystem uvx --args mcp-server-filesystem

# PostgreSQL database
ask add postgres uvx --args mcp-server-postgres

# GitHub API
ask add github uvx --args mcp-server-github
```

### Custom MCP Servers

You can add any MCP server that follows the protocol:

```bash
ask add myserver node --args "/path/to/my/server.js"
ask add myserver python --args "/path/to/my/server.py"
```

## Examples

### Git Workflow

```bash
# Check status
ask what are my pending git changes

# Create commit message
ask generate a commit message for my staged changes

# View history
ask show me the last 5 commits
```

### Code Analysis

```bash
# Understand code
ask explain what main.rs does

# Find patterns
ask find all functions that use async/await

# Generate documentation
ask write documentation for the config module
```

### Project Management

```bash
# Find TODOs
ask list all TODO comments with their file locations

# Analyze structure
ask describe the project structure

# Dependencies
ask what dependencies does this project use
```

## Configuration Files

### File Locations

The configuration file is located at `~/.askrc`

### Example Configuration

See `.askrc.example` for a complete configuration example.

## Development

### Adding Built-in Tools

Edit `src/tools/mod.rs` to add new built-in tools that don't require MCP servers.

### Testing

```bash
# Build
ask build

# Run tests
ask test

# Check
ask check
```

## Troubleshooting

### "No configuration file found"

Create `~/.askrc` or use the `add` command:

```bash
ask add git uvx --args mcp-server-git
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

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see [LICENSE](LICENSE) file for details

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Uses [OpenAI API](https://openai.com/api/)
- Integrates with [MCP (Model Context Protocol)](https://modelcontextprotocol.io/)
- Inspired by [Claude Code](https://claude.com/claude-code)

## Related Projects

- [MCP Servers](https://github.com/modelcontextprotocol/servers) - Official MCP server implementations
- [Claude Code](https://claude.com/claude-code) - Anthropic's AI coding assistant
- [rmcp](https://github.com/modelcontextprotocol/rust-sdk) - Rust MCP SDK
