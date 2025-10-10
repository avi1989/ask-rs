use crate::config;
use crate::config::AskConfig;
use crate::sessions::{get_session, save_session};
use crate::shell::detect_shell_kind;
use crate::tools::mcp::{
    McpRegistry, execute_mcp_tool_call, load_cached_tools, populate_cache_if_needed,
};
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
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::Write;
use std::sync::Mutex;
use tokio::sync::Mutex as AsyncMutex;

/// Track tools that have been auto-approved with "A" (accept all) option
static AUTO_APPROVED_TOOLS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

fn get_api_key(base_url: &Option<String>, verbose: bool) -> Result<String, anyhow::Error> {
    if verbose {
        println!("Checking for API keys...");
        println!("  Base URL: {:?}", base_url);
    }

    if let Ok(key) = env::var("ASK_API_KEY") {
        if verbose {
            println!("  ✓ Found ASK_API_KEY");
        }
        return Ok(key);
    } else if verbose {
        println!("  ✗ ASK_API_KEY not found");
    }

    if let Some(url) = base_url
        && url.contains("openrouter")
    {
        if verbose {
            println!("  Detected OpenRouter URL, checking OPENROUTER_API_KEY...");
        }
        if let Ok(key) = env::var("OPENROUTER_API_KEY") {
            if verbose {
                println!("  ✓ Found OPENROUTER_API_KEY");
            }
            return Ok(key);
        } else if verbose {
            println!("  ✗ OPENROUTER_API_KEY not found");
        }
    }

    if let Ok(key) = env::var("OPENAI_API_KEY") {
        if verbose {
            println!("  ✓ Found OPENAI_API_KEY");
        }
        return Ok(key);
    } else if verbose {
        println!("  ✗ OPENAI_API_KEY not found");
    }

    let error_msg = match base_url {
        Some(url) if url.contains("openrouter") => {
            "No API key found. Please set one of the following environment variables:\n  - ASK_API_KEY (universal)\n  - OPENROUTER_API_KEY (for OpenRouter)\n  - OPENAI_API_KEY (for OpenAI)"
        }
        _ => {
            "No API key found. Please set one of the following environment variables:\n  - ASK_API_KEY (universal)\n  - OPENAI_API_KEY (for OpenAI)\n  - OPENROUTER_API_KEY (if using OpenRouter)"
        }
    };

    Err(anyhow::anyhow!(error_msg))
}
fn get_openai_client(
    base_url: &Option<String>,
    verbose: &bool,
) -> Result<Client<OpenAIConfig>, anyhow::Error> {
    let api_key = get_api_key(base_url, *verbose)?;

    if *verbose {
        println!("Using base URL: {:?}", base_url);
        println!("Successfully initialized OpenAI client");
    }

    let client = match base_url {
        Some(url) => {
            Client::with_config(OpenAIConfig::new().with_api_key(api_key).with_api_base(url))
        }
        None => Client::with_config(OpenAIConfig::new().with_api_key(api_key)),
    };

    Ok(client)
}

pub async fn ask_question(
    question: &str,
    model: Option<String>,
    session: Option<String>,
    verbose: bool,
) -> Result<String, anyhow::Error> {
    let config = config::load_config().unwrap_or_else(|e| {
        if verbose {
            println!("Failed to load MCP config: {e}");
            println!("Continuing without MCP tools. Create ~/.ask/config to enable MCP servers.");
        } else {
            eprintln!("Warning: Failed to load MCP config: {e}");
            eprintln!("Continuing without MCP tools. Create ~/.ask/config to enable MCP servers.");
        }
        AskConfig {
            base_url: None,
            auto_approved_tools: Vec::new(),
            mcp_servers: HashMap::new(),
            model: None,
        }
    });

    if verbose {
        println!("Configuration loaded successfully:");
        println!("  Base URL: {:?}", config.base_url);
        println!("  Default model: {:?}", config.model);
        println!("  MCP servers: {}", config.mcp_servers.len());
        println!(
            "  Auto-approved tools: {}",
            config.auto_approved_tools.len()
        );
    }

    let selected_model = model
        .clone()
        .unwrap_or_else(|| {
            config
                .model
                .as_ref()
                .map_or_else(|| "gpt-4.1-mini".to_string(), |m| m.clone())
        })
        .to_string();

    if verbose {
        println!("Model selection:");
        if let Some(ref provided_model) = model {
            println!("  Using provided model: {}", provided_model);
        } else if let Some(ref config_model) = config.model {
            println!("  Using config default model: {}", config_model);
        } else {
            println!("  Using fallback model: gpt-4.1-mini");
        }
        println!("  Final model: {}", selected_model);
    }

    // Initialize AUTO_APPROVED_TOOLS from config
    {
        let mut auto_approved = AUTO_APPROVED_TOOLS.lock().unwrap();
        for tool in &config.auto_approved_tools {
            auto_approved.insert(tool.clone());
        }
    }

    let client = get_openai_client(&config.base_url, &verbose)?;
    let shell = detect_shell_kind();

    let mut registry = McpRegistry::from_servers(config::config_to_servers(&config));

    // Populate cache if needed (first run only)
    if let Err(e) = populate_cache_if_needed(&mut registry, verbose).await {
        eprintln!("Warning: Failed to populate cache: {e}");
    }

    // Load tools from cache (fast)
    let mut tools = vec![execute_command_tool()];
    tools.extend(load_cached_tools(&registry, verbose));

    let mut messages = match &session {
        Some(session_name) => {
            let session_messages = get_session(session_name);

            match session_messages {
                Some(messages) => messages,
                None => {
                    if verbose {
                        eprintln!("Session not loaded");
                    }
                    get_base_messages(&shell)
                }
            }
        }
        None => get_base_messages(&shell),
    };

    messages.push(
        ChatCompletionRequestUserMessageArgs::default()
            .content(ChatCompletionRequestUserMessageContent::Text(
                question.to_string(),
            ))
            .build()
            .map(ChatCompletionRequestMessage::User)?,
    );

    if verbose {
        println!("Using model: {selected_model}");
    }

    let mut req = CreateChatCompletionRequestArgs::default()
        .model(selected_model.to_string())
        .messages(messages)
        .tools(tools)
        .tool_choice(ChatCompletionToolChoiceOption::Auto)
        .build()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    if verbose {
        println!("Request details:");
        println!("  Model: {}", selected_model);
        println!("  Messages: {} message(s)", req.messages.len());
        println!(
            "  Tools: {} tool(s)",
            req.tools.as_ref().map_or(0, |t| t.len())
        );
    }

    // Wrap registry in async Mutex for interior mutability (safe across await points)
    let registry = AsyncMutex::new(registry);

    for _ in 0..MAX_TURNS {
        let response = match client.chat().create(req.clone()).await {
            Ok(r) => r,
            Err(e) => {
                let error_str = e.to_string();
                if verbose {
                    eprintln!("OpenAI API Error: {}", error_str);
                }

                // Check if it's a model-related error and provide helpful feedback
                if error_str.contains("400") || error_str.contains("invalid type: integer") {
                    return Err(anyhow::anyhow!(
                        "API request failed with 400 error. This might be due to:\n\
                         1. Invalid model name: '{}'\n\
                         2. Request format issues\n\
                         3. API rate limits or permissions\n\n\
                         Original error: {}",
                        selected_model,
                        error_str
                    ));
                }

                return Err(anyhow::anyhow!("OpenAI API Error: {}", error_str));
            }
        };

        let (should_continue, result) = match response.choices[0].finish_reason {
            None => {
                save_session_if_needed(
                    &session,
                    &req.messages,
                    &response.choices[0].message,
                    verbose,
                );

                (
                    false,
                    Some(response.choices[0].message.content.clone().unwrap()),
                )
            }
            Some(FinishReason::Stop) => {
                save_session_if_needed(
                    &session,
                    &req.messages,
                    &response.choices[0].message,
                    verbose,
                );
                (
                    false,
                    Some(response.choices[0].message.content.clone().unwrap()),
                )
            }
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
                None => Err(anyhow::anyhow!("Response too long")),
            };
        } else {
            continue;
        }
    }
    Err(anyhow::anyhow!(format!(
        "No response after {MAX_TURNS} attempts"
    )))
}

fn execute_tool_call(
    tool_call: ChatCompletionMessageToolCall,
    registry: &AsyncMutex<McpRegistry>,
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
                        eprintln!("Warning: Failed to save auto-approval to config: {e}");
                        println!(
                            "All future '{name}' commands will be auto-approved for this session only."
                        );
                    }
                } else if verbose {
                    println!(
                        "All future '{name}' commands will be auto-approved (saved to config)."
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
    } else {
        let server_info = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let reg = registry.lock().await;
                reg.find_server_for_tool(&name)
                    .map(|(name, config)| (name.to_string(), config.clone()))
            })
        });

        if let Some((ref server_name, server_config)) = server_info {
            let is_auto_approved = AUTO_APPROVED_TOOLS.lock().unwrap().contains(&name);

            let should_execute = if is_auto_approved {
                if verbose {
                    let formatted_call = format_mcp_tool_call(&name, &arguments);
                    println!("\n{formatted_call}\n[Auto-approved]");
                }
                true
            } else {
                let formatted_call = format_mcp_tool_call(&name, &arguments);
                print!("\n{formatted_call}\n\nExecute MCP tool '{name}'? [y/N/A]: ");
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
                            eprintln!("Warning: Failed to save auto-approval to config: {e}");
                            println!(
                                "All future '{name}' calls will be auto-approved for this session only."
                            );
                        }
                    } else if verbose {
                        println!(
                            "All future '{name}' calls will be auto-approved (saved to config)."
                        );
                    }
                    true
                } else {
                    trimmed == "y" || trimmed == "yes"
                }
            };

            if should_execute {
                // Initialize server lazily if not already initialized
                let needs_init = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let reg = registry.lock().await;
                        reg.get_service(server_name).is_none()
                    })
                });

                if needs_init {
                    if verbose {
                        eprintln!("Initializing MCP server '{server_name}'...");
                    }

                    let init_result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            let mut reg = registry.lock().await;
                            reg.initialize_service(server_name, verbose).await
                        })
                    });

                    if let Err(e) = init_result {
                        result =
                            format!("Error: Failed to initialize MCP server '{server_name}': {e}",);
                        return (id, result);
                    }
                }

                let service_result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let reg = registry.lock().await;
                        reg.get_service(server_name).is_some()
                    })
                });

                if service_result {
                    let reg = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async { registry.lock().await })
                    });

                    if let Some(service) = reg.get_service(server_name) {
                        match execute_mcp_tool_call(service, &server_config, &name, &arguments) {
                            Ok(response) => {
                                if verbose {
                                    eprintln!("\n[MCP Tool Response]");
                                    eprintln!("{}", response);
                                    eprintln!("[End MCP Tool Response]\n");
                                }
                                result = response;
                            }
                            Err(err) => {
                                result = format!("Error executing MCP tool {name}: {err}");
                            }
                        }
                    } else {
                        result = format!("Error: MCP service '{server_name}' not initialized");
                    }
                } else {
                    result = format!("Error: MCP service '{server_name}' not initialized");
                }
            } else {
                result = "MCP tool execution canceled by user.".to_string();
            }
        } else {
            result = format!("Unknown tool: {name}");
        }
    }

    (id, result)
}

const MAX_TURNS: usize = 21;

fn save_session_if_needed(
    session: &Option<String>,
    messages: &[ChatCompletionRequestMessage],
    response_message: &async_openai::types::ChatCompletionResponseMessage,
    verbose: bool,
) {
    let session_name = session.as_deref().unwrap_or("last");
    match save_session(session_name, messages, Some(response_message)) {
        Ok(_) => {
            if verbose {
                println!("Session saved successfully");
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to save session: {e}");
        }
    }
}

fn format_mcp_tool_call(tool_name: &str, arguments: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(arguments) {
        Ok(json) => {
            let pretty =
                serde_json::to_string_pretty(&json).unwrap_or_else(|_| arguments.to_string());
            format!("MCP Tool: {tool_name}\nArguments:\n{pretty}")
        }
        Err(_) => {
            format!("MCP Tool: {tool_name}\nArguments: {arguments}")
        }
    }
}

fn build_system_prompt(shell: &str) -> String {
    let date = chrono::offset::Local::now().format("%Y-%m-%d").to_string();
    format!(
        "Help the user with their tasks. \n\
         IMPORTANT: This is a one-way conversation - the user cannot reply to your messages.\n\
         Guidelines:\n\
         • You don't need to ask for permission to use the tools available to you \n\
         • Use the current directory as working directory unless otherwise specified\n\
         • Follow the conventions that the user uses.  \n\
            • Example: If the user asks you to generate a commit message, look at other commits and generate a message that is similar to them. \n\
            • If you don't know the answer, try to figure it out based on the information available to you.\n\
         • Ensure shell commands are compatible with {shell}\n\
         • Today's date is {date}.\n\
         • Format all responses in markdown for readability\n\n"
    )
}

fn get_base_messages(shell: &str) -> Vec<ChatCompletionRequestMessage> {
    let system_msg = ChatCompletionRequestSystemMessageArgs::default()
        .content(ChatCompletionRequestSystemMessageContent::Text(
            build_system_prompt(shell),
        ))
        .build()
        .map(ChatCompletionRequestMessage::System)
        .unwrap();

    vec![system_msg]
}
