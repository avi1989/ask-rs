use crate::config;
use crate::shell::detect_shell_kind;
use crate::tools::mcp::{McpRegistry, execute_mcp_tool_call, load_all_mcp_tools};
use crate::tools::{ExecuteCommandRequest, execute_command_tool};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestSystemMessageContent, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestToolMessageContent, ChatCompletionRequestUserMessageArgs,
    ChatCompletionRequestUserMessageContent, ChatCompletionToolChoiceOption,
    CreateChatCompletionRequestArgs, FinishReason,
};
use async_openai::{Client, config::OpenAIConfig};
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::env;
use std::io::Write;
use std::sync::Mutex;

/// Track tools that have been auto-approved with "A" (accept all) option
static AUTO_APPROVED_TOOLS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

fn get_openai_client() -> Client<OpenAIConfig> {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY is not set");
    Client::with_config(OpenAIConfig::new().with_api_key(api_key))
}

pub async fn ask_question(
    question: &str,
    model: &str,
    verbose: bool,
) -> Result<String, Box<anyhow::Error>> {
    let client = get_openai_client();
    let shell = detect_shell_kind();

    let mut registry = match config::load_config() {
        Ok(cfg) => {
            {
                let mut auto_approved = AUTO_APPROVED_TOOLS.lock().unwrap();
                for tool in &cfg.auto_approved_tools {
                    auto_approved.insert(tool.clone());
                }
            }

            let servers = config::config_to_servers(&cfg);
            McpRegistry::from_servers(servers)
        }
        Err(e) => {
            if verbose {
                eprintln!("Warning: Failed to load MCP config: {}", e);
                eprintln!("Continuing without MCP tools. Create ~/.askrc to enable MCP servers.");
            }
            McpRegistry::new()
        }
    };

    // Initialize MCP services once
    if let Err(e) = registry.initialize_services().await {
        eprintln!("Warning: Failed to initialize MCP services: {}", e);
    }

    let mut tools = vec![execute_command_tool()];
    tools.extend(load_all_mcp_tools(&registry, verbose));

    let system_msg = ChatCompletionRequestSystemMessageArgs::default()
        .content(ChatCompletionRequestSystemMessageContent::Text(
            build_system_prompt(&shell),
        ))
        .build()
        .map(ChatCompletionRequestMessage::System)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let user_msg = ChatCompletionRequestUserMessageArgs::default()
        .content(ChatCompletionRequestUserMessageContent::Text(
            question.to_string(),
        ))
        .build()
        .map(ChatCompletionRequestMessage::User)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    let mut req = CreateChatCompletionRequestArgs::default()
        .model(model.to_string())
        .messages(vec![system_msg, user_msg])
        .tools(tools)
        .tool_choice(ChatCompletionToolChoiceOption::Auto)
        .build()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    for _ in 0..MAX_TURNS {
        let response = match client.chat().create(req.clone()).await {
            Ok(r) => r,
            Err(e) => return Err(Box::from(anyhow::anyhow!(e.to_string()))),
        };

        let (should_continue, result) = match response.choices[0].finish_reason {
            None => (
                false,
                Some(response.choices[0].message.content.clone().unwrap()),
            ),
            Some(FinishReason::Stop) => (
                false,
                Some(response.choices[0].message.content.clone().unwrap()),
            ),
            Some(FinishReason::Length) => (false, None),
            Some(FinishReason::ToolCalls) => {
                let tool_calls = response.choices[0].message.tool_calls.clone().unwrap();

                let assistant_msg = ChatCompletionRequestAssistantMessageArgs::default()
                    .tool_calls(tool_calls.clone())
                    .build()
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                req.messages
                    .push(ChatCompletionRequestMessage::Assistant(assistant_msg));

                for tool_call in tool_calls {
                    let (id, result) = execute_tool_call(tool_call, &registry, verbose);
                    let tool_msg = ChatCompletionRequestToolMessageArgs::default()
                        .tool_call_id(id)
                        .content(ChatCompletionRequestToolMessageContent::Text(result))
                        .build()
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    req.messages
                        .push(ChatCompletionRequestMessage::Tool(tool_msg));
                }

                (true, None)
            }
            _ => (false, None),
        };

        if !should_continue {
            return match result {
                Some(r) => Ok(r),
                None => Err(Box::from(anyhow::anyhow!("Response too long"))),
            };
        } else {
            continue;
        }
    }
    Err(Box::from(anyhow::anyhow!(format!(
        "No response after {} attempts",
        MAX_TURNS
    ))))
}

fn execute_tool_call(
    tool_call: ChatCompletionMessageToolCall,
    registry: &McpRegistry,
    verbose: bool,
) -> (String, String) {
    let name = tool_call.function.name.clone();
    let arguments = tool_call.function.arguments.clone();
    let id = tool_call.id.clone();
    let result: String;

    if name == "execute_command" {
        let args: ExecuteCommandRequest = serde_json::from_str(&arguments).unwrap();

        let is_auto_approved = AUTO_APPROVED_TOOLS.lock().unwrap().contains(&name);

        let should_execute = if is_auto_approved {
            if verbose {
                println!("{}\n[Auto-approved]", args.command);
            }
            true
        } else {
            print!(
                "{}\nCan I execute the above command? [y/N/A]: ",
                args.command
            );
            std::io::stdout().flush().expect("Failed to flush stdout");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read user input");
            let trimmed = input.trim().to_lowercase();

            if trimmed == "a" || trimmed == "all" {
                AUTO_APPROVED_TOOLS.lock().unwrap().insert(name.clone());
                if let Err(e) = config::add_auto_approved_tool(&name) {
                    if verbose {
                        eprintln!("Warning: Failed to save auto-approval to config: {}", e);
                        println!(
                            "All future '{}' commands will be auto-approved for this session only.",
                            name
                        );
                    }
                } else if verbose {
                    println!(
                        "All future '{}' commands will be auto-approved (saved to config).",
                        name
                    );
                }
                true
            } else {
                trimmed == "y" || trimmed == "yes"
            }
        };

        if should_execute {
            let cmd_result = crate::tools::execute_command(&args.command, &args.working_directory);
            result = if cmd_result.is_empty() {
                "Executed".to_string()
            } else {
                cmd_result
            };
        } else {
            result = "Command execution canceled by user.".to_string();
        }
    } else if let Some((server_name, server_config)) = registry.find_server_for_tool(&name) {
        let is_auto_approved = AUTO_APPROVED_TOOLS.lock().unwrap().contains(&name);

        let should_execute = if is_auto_approved {
            if verbose {
                let formatted_call = format_mcp_tool_call(&name, &arguments);
                println!("\n{}\n[Auto-approved]", formatted_call);
            }
            true
        } else {
            let formatted_call = format_mcp_tool_call(&name, &arguments);
            print!(
                "\n{}\n\nExecute MCP tool '{}'? [y/N/A]: ",
                formatted_call, name
            );
            std::io::stdout().flush().expect("Failed to flush stdout");

            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read user input");
            let trimmed = input.trim().to_lowercase();

            if trimmed == "a" || trimmed == "all" {
                AUTO_APPROVED_TOOLS.lock().unwrap().insert(name.clone());
                if let Err(e) = config::add_auto_approved_tool(&name) {
                    if verbose {
                        eprintln!("Warning: Failed to save auto-approval to config: {}", e);
                        println!(
                            "All future '{}' calls will be auto-approved for this session only.",
                            name
                        );
                    }
                } else if verbose {
                    println!(
                        "All future '{}' calls will be auto-approved (saved to config).",
                        name
                    );
                }
                true
            } else {
                trimmed == "y" || trimmed == "yes"
            }
        };

        if should_execute {
            if let Some(service) = registry.get_service(server_name) {
                match execute_mcp_tool_call(service, server_config, &name, &arguments) {
                    Ok(response) => {
                        result = response;
                    }
                    Err(err) => {
                        result = format!("Error executing MCP tool {}: {}", name, err);
                    }
                }
            } else {
                result = format!("Error: MCP service '{}' not initialized", server_name);
            }
        } else {
            result = "MCP tool execution canceled by user.".to_string();
        }
    } else {
        result = format!("Unknown tool: {}", name);
    }

    (id, result)
}

const MAX_TURNS: usize = 21;

fn format_mcp_tool_call(tool_name: &str, arguments: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(arguments) {
        Ok(json) => {
            let pretty =
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| arguments.to_string());
            format!("MCP Tool: {}\nArguments:\n{}", tool_name, pretty)
        }
        Err(_) => {
            format!("MCP Tool: {}\nArguments: {}", tool_name, arguments)
        }
    }
}

fn build_system_prompt(shell: &str) -> String {
    format!(
        "You are an AI assistant with access to powerful tools through MCP (Model Context Protocol) servers and built-in capabilities.\n\n\
        IMPORTANT: This is a one-way conversation - the user cannot reply to your messages.\n\n\
        Guidelines:\n\
        • Use available tools to provide comprehensive assistance\n\
        • Ensure shell commands are compatible with {shell}\n\
        • Use the current directory as working directory unless otherwise specified\n\
        • Format all responses in markdown for readability\n\n\
        You can help with development tasks, file operations, git workflows, system administration, and any functionality provided by configured MCP servers."
    )
}
