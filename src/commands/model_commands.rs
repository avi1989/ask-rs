use crate::commands::cli::ModelCommands;
use crate::config::{load_config, set_default_model};

pub fn handle_model_commands(model_commands: ModelCommands) {
    match model_commands {
        ModelCommands::Get => {
            let config = load_config();
            match config {
                Ok(config) => {
                    println!("{}", config.model.unwrap_or("gpt-4.1-mini".to_string()))
                }
                Err(_) => {
                    println!("Unable to load default model");
                }
            }
        }
        ModelCommands::Set { model } => {
            let _ = set_default_model(&model);
            println!("Default model set to {}", model);
        }
    }
}
