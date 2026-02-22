use crate::{
    commands::cli::BaseUrlCommands,
    config::{self},
};

pub fn handle_base_url_commands(command: BaseUrlCommands) {
    match command {
        BaseUrlCommands::Get => {
            handle_get();
        }
        BaseUrlCommands::Set { base_url } => handle_set(&base_url),
        BaseUrlCommands::Remove => {
            handle_remove();
        }
    }
}

fn handle_get() {
    match config::load_config() {
        Ok(cfg) => {
            if let Some(base_url) = cfg.base_url {
                println!("Current base URL: {base_url}");
            } else {
                println!("No Base URL configured")
            }
        }
        Err(_e) => {
            println!("No Base URL configured")
        }
    }
}

fn handle_set(base_url: &str) {
    match config::set_base_url(base_url) {
        Ok(_url) => {
            println!("Base URL updated")
        }
        Err(e) => {
            eprintln!("Failed to set baseUrl. {e}")
        }
    }
}

fn handle_remove() {
    match config::remove_base_url() {
        Ok(_cfg) => {
            println!("Base URL removed")
        }
        Err(e) => {
            eprintln!("Unable to remove base url {e}")
        }
    }
}
