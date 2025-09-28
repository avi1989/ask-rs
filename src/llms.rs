use crate::tools::{ListFilesRequest, ListFilesToolRequest, list_all_files, list_all_files_tool, read_file, read_file_tool, execute_command_tool, ExecuteCommandRequest};
use anyhow;
use openai_api_rs::v1::chat_completion::{ChatCompletionMessage, ToolCall};
use openai_api_rs::v1::{
    api::OpenAIClient,
    chat_completion::{self, ChatCompletionRequest},
};
use serde::{Deserialize, Serialize};
use std::env;
use crate::shell::detect_shell_kind;

fn get_openai_client() -> OpenAIClient {
    let api_key = env::var("OPENAI_API_KEY").unwrap().to_string();
    OpenAIClient::builder()
        .with_api_key(api_key)
        .build()
        .unwrap()
}

pub async fn ask_question(question: &str) -> Result<String, Box<anyhow::Error>> {
    let mut client = get_openai_client();
    let model = "gpt-4.1".to_string();
    let shell = detect_shell_kind();
    let mut req = ChatCompletionRequest::new(
        model,
        vec![ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(format!("\
            You are a terminal assistant to the user. \
            The user cannot reply to your messages; \
            this is a one-way conversation. \n\
            The current shell is {}. Make sure that commands generated \
            apply to this shell.
            ", shell).to_string()),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }, ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(question.to_string()),
            name: None,
            tool_call_id: None,
            tool_calls: None
        }],
    ).tools(vec![list_all_files_tool(), read_file_tool(), execute_command_tool()]).tool_choice(chat_completion::ToolChoiceType::Auto);

    for _i in 0..10 {
        let response_result = client.chat_completion(req.clone()).await;
        let response = response_result.unwrap();

        let (should_continue, result) = match response.choices[0].finish_reason {
            None => {
                println!("{:?}", response.choices[0].message.content);
                (
                    false,
                    Some(response.choices[0].message.content.clone().unwrap()),
                )
            }
            Some(chat_completion::FinishReason::stop) => (
                false,
                Some(response.choices[0].message.content.clone().unwrap()),
            ),
            Some(chat_completion::FinishReason::length) => (false, None),
            Some(chat_completion::FinishReason::tool_calls) => {
                let tool_calls = response.choices[0].message.tool_calls.clone().unwrap();
                req.messages.push(ChatCompletionMessage {
                    role: chat_completion::MessageRole::assistant,
                    content: chat_completion::Content::Text(String::new()),
                    tool_calls: Some(tool_calls.clone()),
                    name: None,
                    tool_call_id: None,
                });
                for tool_call in tool_calls {
                    let (id, result) = execute_tool_call(tool_call);
                    req.messages.push(ChatCompletionMessage {
                        tool_call_id: Some(id),
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(result),
                        name: None,
                        tool_calls: None,
                    });
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
    Err(Box::from(anyhow::anyhow!("No response after 10 attempts")))
}

fn execute_tool_call(tool_call: ToolCall) -> (String, String) {
    let name = tool_call.function.name.clone().unwrap();
    let arguments = tool_call.function.arguments.unwrap();
    let id = tool_call.id;
    let mut result: String = String::new();
    if name == "execute_command" {
        let args: ExecuteCommandRequest = serde_json::from_str(&arguments).unwrap();
        println!("Command: {}", args.command);
        print!("Do you want to allow this command? (y/n): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("Failed to read user input");
        if input.trim().to_lowercase() == "y" || input.trim().to_lowercase() == "yes" {
            // Execute the command only if user approves
            result = crate::tools::execute_command(&args.command, &args.working_directory);
            if result == "" {
                result = "Executed".to_string()
            }
        } else {
            result = "Command execution canceled by user.".to_string();
        }
    }
    else if name == "list_all_files" {
        let args: ListFilesRequest = serde_json::from_str(&arguments).unwrap();
        let files = list_all_files(args.base_path.as_str());
        for file in files {
            result.push_str(&file);
            result.push('\n');
        }
    } else if name == "read_file" {
        let args: ListFilesToolRequest = serde_json::from_str(&arguments).unwrap();
        result = read_file(args.file_path.as_str());
        result.push('\n');
    }

    (id, result)
}
