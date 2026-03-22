use crate::commands::cli::Presets;
use crate::config::{self, list_prompt_presets, remove_prompt_presets};

pub fn handle_preset_commands(command: Presets) {
    match command {
        Presets::Add { name, prompt } => handle_add(name, prompt),
        Presets::List => handle_list(),
        Presets::Remove { name } => handle_remove(name),
    }
}

fn handle_add(name: String, prompt_rest: Vec<String>) {
    let prompt = prompt_rest.join(" ");
    match config::add_prompt_presets(name, prompt) {
        Ok(_res) => {
            println!("Prompt preset added")
        }
        Err(err) => {
            eprintln!("{err}")
        }
    }
}

fn handle_list() {
    match list_prompt_presets() {
        Ok(_res) => {}
        Err(err) => {
            eprintln!("{err}")
        }
    }
}

fn handle_remove(name: String) {
    match remove_prompt_presets(name) {
        Ok(_res) => {
            println!("Prompt preset removed")
        }
        Err(err) => {
            eprintln!("{err}")
        }
    }
}
