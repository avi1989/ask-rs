use crate::config;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::io::Write;
use std::sync::Mutex;

static AUTO_APPROVED_TOOLS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, PartialEq)]
pub enum ApprovalResponse {
    Yes,
    No,
    AutoApprove,
}

pub fn initialize_from_config(tools: &[String]) {
    let mut approved = AUTO_APPROVED_TOOLS.lock().unwrap();
    for tool in tools {
        approved.insert(tool.clone());
    }
}

pub fn is_auto_approved(tool_name: &str) -> bool {
    AUTO_APPROVED_TOOLS.lock().unwrap().contains(tool_name)
}

fn add_to_session_auto_approved(tool_name: &str) {
    AUTO_APPROVED_TOOLS
        .lock()
        .unwrap()
        .insert(tool_name.to_string());
}

fn prompt_user_approval(prompt_message: &str, tool_name: &str) -> ApprovalResponse {
    print!("{}\nExecute '{}'? [y/N/A]: ", prompt_message, tool_name);

    if let Err(e) = std::io::stdout().flush() {
        eprintln!("Warning: Failed to flush stdout: {}", e);
    }

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut input) {
        eprintln!("Error: Failed to read user input: {}", e);
        return ApprovalResponse::No;
    }

    let trimmed = input.trim().to_lowercase();
    match trimmed.as_str() {
        "y" | "yes" => ApprovalResponse::Yes,
        "a" | "all" => ApprovalResponse::AutoApprove,
        _ => ApprovalResponse::No,
    }
}

pub fn check_approval(tool_name: &str, prompt_message: &str, verbose: bool) -> bool {
    if is_auto_approved(tool_name) {
        if verbose {
            println!("{}\n[Auto-approved]", prompt_message);
        } else {
            println!("{}", prompt_message);
        }
        return true;
    }

    match prompt_user_approval(prompt_message, tool_name) {
        ApprovalResponse::Yes => true,
        ApprovalResponse::No => false,
        ApprovalResponse::AutoApprove => {
            add_to_session_auto_approved(tool_name);

            if let Err(e) = config::add_auto_approved_tool(tool_name) {
                if verbose {
                    eprintln!("Warning: Failed to save auto-approval to config: {}", e);
                    println!(
                        "All future '{}' calls will be auto-approved for this session only.",
                        tool_name
                    );
                }
            } else if verbose {
                println!(
                    "All future '{}' calls will be auto-approved (saved to config).",
                    tool_name
                );
            }

            true
        }
    }
}
