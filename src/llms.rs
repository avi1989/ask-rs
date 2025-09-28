use std::env;

use openai_api_rs::v1::{api::OpenAIClient, chat_completion::{self, ChatCompletionRequest}};

fn get_openai_client() -> OpenAIClient {
    let api_key = env::var("OPENAI_API_KEY").unwrap().to_string();
    OpenAIClient::builder().with_api_key(api_key).build().unwrap()
}

pub async fn ask_question(question: &str) -> String {
    let mut client = get_openai_client();
    let model = "gpt-4.1".to_string();
    let req = ChatCompletionRequest::new(
        model,
        vec![chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(question.to_string()),
            name: None,
            tool_call_id: None,
            tool_calls: None
        }]
    );

    let response = client.chat_completion(req).await;
    match response {
        Ok(completion) => {
            if let Some(choice) = completion.choices.first() {
                if let Some(content) = &choice.message.content {
                    content.clone()
                } else {
                    "No content in response".to_string()
                }
            } else {
                "No choices in response".to_string()
            }
        },
        Err(e) => {
            format!("Error: {:?}", e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ask_question() {
        let question = "What is the capital of France?";
        let answer = ask_question(question).await;
        println!("Answer: {}", answer);
        assert!(answer.to_lowercase().contains("paris"));
    }
}