use crate::commands::cli::ModelCommands;
use crate::config::{load_config, save_config, set_default_model};

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
        ModelCommands::Aliases => {
            let config = load_config().expect("Failed to load config");
            let aliases = config.model_aliases;
            if aliases.is_empty() {
                println!("No model aliases configured");
            } else {
                for (alias, model) in aliases {
                    println!("{}: {}", alias, model);
                }
            }
        }
        ModelCommands::Alias { alias, model } => {
            let mut config = load_config().expect("Failed to load config");
            config.model_aliases.insert(alias.clone(), model.clone());
            let _ = save_config(&config);
            println!("Model alias {} set to {}", alias, model);
        }
        ModelCommands::Unalias { alias } => {
            let mut config = load_config().expect("Failed to load config");
            if !config.model_aliases.contains_key(&alias) {
                println!("Model alias {} not found", alias);
                return;
            }
            config.model_aliases.remove(&alias);
            let _ = save_config(&config);
            println!("Model alias removed");
        }
    }
}
