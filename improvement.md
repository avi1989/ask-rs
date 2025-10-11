# CLI Interface Improvement Suggestions for `ask`

This document outlines comprehensive suggestions to improve the user interface and experience of the `ask` CLI tool.

## 1. Command Structure Reorganization

### Current Issues
- Inconsistent command hierarchy (some top-level, some nested)
- `save-last-session` is top-level while `session list/show` are nested
- `set-base-url` and `set-default-model` are top-level but should be grouped
- `mcp` is not a user-friendly name

### Suggested New Structure

```bash
# Asking questions (default)
ask [question]                    # Ask a question
ask --reply [question]            # Reply to last conversation
ask --continue [question]         # Alias for --reply
ask --session <name> [question]   # Continue a named session

# Configuration management
ask config show                   # Show current configuration
ask config set base-url <url>     # Set base URL (instead of set-base-url)
ask config set model <model>      # Set default model (instead of set-default-model)
ask config init                   # Initialize with defaults (instead of init)
ask config path                   # Show config file location
ask config edit                   # Open config in $EDITOR

# MCP Server management (rename mcp -> server)
ask server list                   # List MCP servers (instead of mcp list)
ask server add <name> ...         # Add MCP server (instead of mcp add)
ask server remove <name>          # Remove MCP server (instead of mcp remove)
ask server test <name>            # Test a server connection (NEW)

# Session management (consolidate)
ask session list                  # List sessions
ask session show [name]           # Show session (defaults to last)
ask session save <name>           # Save last session (instead of save-last-session)
ask session delete <name>         # Delete a session (NEW)
ask session rename <old> <new>    # Rename a session (NEW)
ask session clear                 # Clear all sessions (NEW)

# Tool approval management (rename to approval)
ask approval list                 # List auto-approved tools (instead of mcp approvals)
ask approval add <tool>           # Add auto-approval (instead of mcp approve)
ask approval remove <tool>        # Remove auto-approval (instead of mcp unapprove)
ask approval clear                # Clear all approvals (NEW)

# Help and examples
ask examples                      # Show common usage examples (NEW)
ask --help                        # Comprehensive help
ask <subcommand> --help          # Subcommand-specific help
```

### Migration Guide for Users

Create an alias system or deprecation warnings:
```
$ ask mcp list
⚠ Warning: 'ask mcp' is deprecated. Use 'ask server' instead.
  Example: ask server list
```

## 2. Improved User Prompts and Confirmations

### Current State
```
Execute MCP tool 'git_status'? [y/N/A]:
```

### Suggested Improvements

#### For MCP Tool Calls
```
┌─ MCP Tool Call ──────────────────────────┐
│ Tool: git_status                         │
│ Server: git                              │
│                                          │
│ Arguments:                               │
│   repo_path: "."                         │
└──────────────────────────────────────────┘

Execute this tool?
  [y] Yes, once    [a] Always approve    [n] No (default)
Choice: _
```

#### For Command Execution
```
┌─ Command Execution ──────────────────────┐
│ $ git status                             │
│                                          │
│ Working directory: /current/path         │
└──────────────────────────────────────────┘

Execute this command?
  [y] Yes, once    [a] Always approve    [n] No (default)
Choice: _
```

#### For Configuration Changes
```
┌─ Configuration Change ───────────────────┐
│ Setting: base_url                        │
│ New value: https://api.openrouter.ai    │
└──────────────────────────────────────────┘

Continue? [Y/n]: _
```

### Standardize All Prompts
- Use consistent box drawing characters
- Show context (what's being executed, where, why)
- Clear option descriptions
- Consistent capitalization and formatting

## 3. Enhanced Output Formatting

### Color Coding
```rust
// Suggested color scheme:
- User messages:        Cyan (36m)
- Assistant messages:   Green (32m)
- Tool calls:           Yellow (33m)
- Errors:               Red (31m)
- Success:              Green (32m)
- Warnings:             Yellow (33m)
- Info:                 Blue (34m)
```

### Icons and Symbols
```
✓ Success operations
✗ Errors
⚠ Warnings
ℹ Informational messages
⟳ In progress / Loading
⋯ Pending
→ Processing step
```

### Examples
```bash
$ ask server add git uvx --args mcp-server-git
⟳ Adding MCP server 'git'...
✓ Added MCP server 'git' to ~/.ask/config
ℹ Run 'ask server list' to see all servers

$ ask server remove nonexistent
✗ Error: Server 'nonexistent' not found
ℹ Available servers: git, filesystem
```

## 4. Better Session Management

### Enhanced Session List Output

#### Current
```
debugging-issue 2 hours ago
feature-work 1 day ago
```

#### Suggested
```
Sessions:
┌──────────────────┬───────────────────┬──────────┬──────────────┐
│ Name             │ Last Modified     │ Messages │ Model        │
├──────────────────┼───────────────────┼──────────┼──────────────┤
│ debugging-issue  │ 2 hours ago       │ 15       │ gpt-4        │
│ feature-work     │ yesterday         │ 8        │ gpt-4.1-mini │
│ last             │ 5 minutes ago     │ 3        │ gpt-4.1-mini │
└──────────────────┴───────────────────┴──────────┴──────────────┘

Total: 3 sessions
Use 'ask session show <name>' to view a session
Use 'ask --session <name> "question"' to continue a session
```

### New Session Commands
```bash
# Delete a session
ask session delete debugging-issue

# Rename a session
ask session rename last my-work

# Clear all sessions (with confirmation)
ask session clear
⚠ This will delete all 3 sessions. Continue? [y/N]:

# Show session statistics
ask session info <name>
Session: debugging-issue
Created: 2024-10-10 14:30
Last modified: 2 hours ago
Messages: 15 (8 user, 7 assistant)
Tools used: git_status (3x), read_file (5x)
Model: gpt-4
```

## 5. Configuration Management Improvements

### New Config Subcommands

```bash
# Show all current settings
ask config show
Configuration (~/.ask/config):
  Base URL: https://api.openai.com/v1
  Default Model: gpt-4.1-mini
  MCP Servers: 2 (git, filesystem)
  Auto-approved tools: 3 (git_status, git_log, git_diff)

# Show config file path
ask config path
~/.ask/config

# Open config in editor
ask config edit
# Opens ~/.ask/config in $EDITOR

# Set values with better validation
ask config set base-url https://api.openrouter.ai
✓ Updated base_url to https://api.openrouter.ai
ℹ You may need to set OPENROUTER_API_KEY

# Show specific config value
ask config get base-url
https://api.openrouter.ai
```

## 6. Progress Indicators

### For Long-Running Operations

Use the `indicatif` crate for spinners and progress bars:

```bash
$ ask "complex question requiring many tools"
⠋ Waiting for AI response...
⠙ Initializing MCP server 'git'...
✓ git_status completed
⠹ Executing read_file...
✓ read_file completed
⠸ Generating response...
✓ Done!

[Response shows here]
```

### For File Operations
```bash
$ ask "read all rust files and summarize"
Reading files: ████████████████████ 45/45 (100%)
✓ Analyzed 45 files (12,345 lines)
```

## 7. Better Error Messages and Guidance

### Current
```
Error: No configuration file found
```

### Suggested
```
✗ Error: No configuration file found at ~/.ask/config

To get started, run:
  ask config init              Initialize with default servers

Or create the config manually:
  ask config edit              Open config in editor

For examples and help:
  ask examples                 Show common usage examples
  ask --help                   Show all commands
```

### More Examples

#### Missing API Key
```
✗ Error: No API key found

Please set one of the following environment variables:
  • ASK_API_KEY (universal)
  • OPENAI_API_KEY (for OpenAI)
  • OPENROUTER_API_KEY (for OpenRouter)

Example:
  export OPENAI_API_KEY="sk-..."

Current base URL: https://api.openai.com/v1
Need to change provider? Use: ask config set base-url <url>
```

#### Server Connection Error
```
✗ Error: Failed to connect to MCP server 'git'

Possible issues:
  1. Server command not found: uvx
     Install with: pip install uvx

  2. Check server configuration:
     ask server list

  3. Test server manually:
     uvx mcp-server-git

For help: ask server --help
```

## 8. Quality of Life Improvements

### Additional Flags

```bash
# Quick model override
ask --model gpt-4 "complex question"
ask -m o1-preview "reasoning task"

# Continue last session (alias)
ask --continue "what about tomorrow?"
ask -c "and next week?"

# Disable tools for faster responses
ask --no-tools "simple question"

# Dry run mode (show what would happen)
ask --dry-run "dangerous operation"
🔍 Dry run mode - no changes will be made
[Shows what tools would be called]

# Edit last message
ask --edit
# Opens $EDITOR with last user message
# On save, resends the edited message

# JSON output for scripting
ask --json "what's the date?"
{"role": "assistant", "content": "2024-10-11", ...}

# Quiet mode (only show final answer)
ask --quiet "quick question"
```

### Better Stdin Handling

```bash
# Pipe input with clear context
cat error.log | ask "what's wrong with this?"
cat README.md | ask "summarize this"
git diff | ask "review this change"

# When stdin is detected, show indicator
$ cat file.txt | ask "explain"
📎 Stdin detected (1,234 bytes)
⟳ Processing...
```

### Interactive Mode

```bash
$ ask --interactive
╔════════════════════════════════════════╗
║  Ask - AI Assistant (Interactive)      ║
║  Type 'help' for commands, 'exit' to quit ║
╚════════════════════════════════════════╝

Using model: gpt-4.1-mini
Session: interactive-2024-10-11

You: what files are here?
Assistant: [response]

You: /save my-session
✓ Saved session as 'my-session'

You: /model gpt-4
✓ Switched to model: gpt-4

You: /help
Available commands:
  /save <name>     Save current session
  /model <name>    Switch model
  /clear           Clear conversation
  /verbose         Toggle verbose mode
  /exit            Exit interactive mode

You: /exit
✓ Saved session as 'last'
```

## 9. Shell Completion Support

### Implementation

Using Clap's built-in completion generation:

```bash
# Generate completions for different shells
ask completion bash > /etc/bash_completion.d/ask
ask completion zsh > ~/.zsh/completions/_ask
ask completion fish > ~/.config/fish/completions/ask.fish
ask completion powershell > ask.ps1
```

### Benefits
- Tab-complete subcommands
- Tab-complete session names
- Tab-complete server names
- Tab-complete tool names for approval

## 10. Examples Command

Add a new command to show common usage patterns:

```bash
$ ask examples

Common Usage Examples:
═══════════════════════════════════════════════════════════

Getting Started:
  ask config init                       # Set up with defaults
  ask "hello world"                     # Ask your first question

Git Workflows:
  ask "what are my pending changes?"    # Check git status
  ask "generate a commit message"       # AI commit message
  ask "show me the last 5 commits"      # View history

Code Analysis:
  ask "explain main.rs"                 # Understand code
  cat file.rs | ask "review this"       # Code review
  ask "find all TODO comments"          # Find patterns

Sessions:
  ask "start new feature"               # Begin conversation
  ask --reply "continue that work"      # Continue last session
  ask --session mywork "status?"        # Named session
  ask session save feature-x            # Save for later

Piping:
  cat error.log | ask "what's wrong?"   # Analyze logs
  git diff | ask "review this change"   # Review changes
  ls -la | ask "explain these files"    # Explain output

Configuration:
  ask server add git uvx --args mcp-server-git
  ask approval add git_status           # Auto-approve tool
  ask config set model gpt-4            # Change default model

More help:
  ask --help                            # All commands
  ask <command> --help                  # Command-specific help
```

## 11. Verbose and Debug Output

### Improved Logging Levels

```bash
# Default: Show essential information
ask "question"

# Quiet: Only show final answer
ask --quiet "question"
ask -q "question"

# Verbose: Show tool calls and operations
ask --verbose "question"
ask -v "question"
⟳ Loading configuration...
✓ Found config at ~/.ask/config
✓ Loaded 2 MCP servers
⟳ Initializing git server...
✓ Connected to git server
⟳ Sending request to gpt-4.1-mini...
[Response]
✓ Session saved as 'last'

# Debug: Show everything including API details
ask --debug "question"
ask -vv "question"
[DEBUG] API Request:
  Model: gpt-4.1-mini
  Messages: 2
  Tools: 15
[DEBUG] API Response:
  Finish reason: tool_calls
  Tool: git_status
[DEBUG] Tool execution time: 234ms
```

## 12. Smart Defaults and Auto-initialization

### First-time User Experience

```bash
# User's first command
$ ask "hello"

⚠ No configuration found. Would you like to initialize? [Y/n]: y

⟳ Checking for required commands...
✓ Found npx (for filesystem server)
✓ Found uvx (for git server)

⟳ Creating ~/.ask/config...
✓ Added filesystem server
✓ Added git server
✓ Configuration created

🤖 Asking your question...
[Response]

Next steps:
  • Set your API key: export OPENAI_API_KEY="sk-..."
  • View servers: ask server list
  • See examples: ask examples
```

### Smart Session Handling

```bash
# Automatically detect when to use sessions
$ ask "what's the weather?"
[No session needed - standalone question]

$ ask --reply "and tomorrow?"
✓ Continuing last session
[Uses context from previous question]

$ ask --session project "status update"
✓ Loaded session 'project' (12 previous messages)
[Response with full context]
```

## 13. Tool Call Visualization

### Show Tool Chain Execution

```bash
$ ask "analyze all rust files and create summary"

┌─ Tool Chain ─────────────────────────────────────┐
│ 1. ✓ list_directory (.) - Found 45 files        │
│ 2. ✓ read_file (src/main.rs) - 234 lines        │
│ 3. ✓ read_file (src/config.rs) - 156 lines      │
│ 4. ⟳ read_file (src/llms.rs) - Reading...       │
│ 5. ⋯ Pending: 42 more files                     │
└──────────────────────────────────────────────────┘

Progress: ████████░░░░░░░░░░░░ 3/45 (7%)
```

### Summary After Execution

```bash
✓ Completed in 5.2 seconds

Tool Usage Summary:
  list_directory: 1 call
  read_file: 45 calls

Total tokens: 12,345
Estimated cost: $0.024
```

## 14. Alias Support

Add common aliases as actual command aliases or document them:

```bash
# In help text or documentation
Common Aliases:
  ask q "..."              → ask "..."
  ask s list               → ask session list
  ask s show               → ask session show
  ask cfg show             → ask config show
  ask srv list             → ask server list
```

Or implement actual alias commands:
```rust
#[command(alias = "q")]
Question,

#[command(alias = "s")]
Session,

#[command(alias = "cfg")]
Config,

#[command(alias = "srv")]
Server,
```

## 15. Cross-Platform Consistency

### Windows Considerations

```bash
# Ensure proper NPX detection (already implemented)
- Check for both npx and npx.cmd
- Use proper path handling

# Better error messages for Windows
✗ Error: 'uvx' command not found

Install Python UV:
  Windows: powershell -c "irm https://astral.sh/uv/install.ps1 | iex"
  Linux/Mac: curl -LsSf https://astral.sh/uv/install.sh | sh

After installation, restart your terminal.
```

### Path Handling
- Always use `std::path::PathBuf` for paths
- Use `shellexpand` for tilde expansion (already done)
- Handle both forward and backslashes on Windows

## Implementation Priority

### High Priority (Core UX improvements)
1. Reorganize command structure (config, server, session, approval groups)
2. Improve error messages with actionable guidance
3. Add success/error icons and colors
4. Better confirmation prompts
5. Add `config show` command

### Medium Priority (Nice to have)
6. Progress indicators for long operations
7. Shell completion support
8. `examples` command
9. Enhanced session list output
10. Session delete/rename commands
11. Smart auto-initialization

### Low Priority (Polish)
12. Interactive mode
13. Tool chain visualization
14. Detailed execution summaries
15. Command aliases
16. `--edit` flag for editing last message

## Backward Compatibility

### Deprecation Strategy

1. Keep old commands working but show deprecation warnings
2. Add `--no-warnings` flag to suppress deprecation messages
3. Document migration path in README
4. Eventually remove old commands in major version bump

```bash
# Example deprecation warning
$ ask mcp list
⚠ Warning: 'ask mcp' is deprecated and will be removed in v2.0
  Use: ask server list

# Suppress warnings
$ ask --no-warnings mcp list
```

## Testing Checklist

- [ ] All new commands have help text
- [ ] All error messages provide next steps
- [ ] Colors work in different terminal types
- [ ] Box drawing characters work cross-platform
- [ ] Shell completions work for bash/zsh/fish
- [ ] Deprecation warnings are clear
- [ ] Examples are tested and accurate
- [ ] Stdin piping works correctly
- [ ] Windows-specific commands work (npx.cmd, paths)
- [ ] Progress indicators don't interfere with output

## Conclusion

These improvements focus on:
- **Consistency**: Unified command structure and naming
- **Discoverability**: Better help, examples, and error messages
- **User Experience**: Visual improvements, progress indicators, smart defaults
- **Power Features**: Shell completion, interactive mode, advanced flags

The goal is to make `ask` feel polished, professional, and intuitive for both new and experienced users.
