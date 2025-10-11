use crate::commands::SessionCommands;
use crate::sessions::{get_all_sessions, get_last_session_name, get_session, save_session};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestUserMessageContent,
};
use crossterm::terminal;

pub fn handle_session_commands(command: SessionCommands) {
    match command {
        SessionCommands::List => match get_all_sessions() {
            Ok(sessions) => {
                for session in sessions {
                    println!("{:<20} {}", session.name, session.created);
                }
            }
            Err(e) => {
                eprintln!("Error: Failed to list sessions: {}", e);
                std::process::exit(1);
            }
        },
        SessionCommands::Show { name } => {
            let name = name
                .unwrap_or_else(|| get_last_session_name().unwrap_or_else(|| "last".to_string()));
            handle_show_session(name);
        }
        SessionCommands::Save { name } => match get_session("last") {
            Some(session) => match save_session(&name, &session, None) {
                Ok(_) => println!("Saved session as {name}"),
                Err(e) => {
                    eprintln!("Error: Failed to save session: {}", e);
                    std::process::exit(1);
                }
            },
            None => {
                eprintln!("Error: No session to save");
                std::process::exit(1);
            }
        },
    }
}

struct MessageBoxConfig {
    label: &'static str,
    color: &'static str,
    max_width_percent: f32,
    align_right: bool,
    left_margin: usize,
}

fn handle_show_session(name: String) {
    use std::fmt::Write as FmtWrite;

    let session = get_session(&name);
    match session {
        Some(session) => {
            let is_interactive = atty::is(atty::Stream::Stdout);
            let (width, _) = terminal::size().unwrap_or((80, 24));
            let mut output = String::new();

            writeln!(&mut output).unwrap();

            // Display session name header (centered in interactive mode)
            if is_interactive {
                let header_text = format!("═══ Session: {} ═══", name);
                let header_len = header_text.chars().count();
                let left_padding = if header_len < width as usize {
                    (width as usize - header_len) / 2
                } else {
                    0
                };
                writeln!(
                    &mut output,
                    "{}\x1b[1m\x1b[35m{}\x1b[0m",
                    " ".repeat(left_padding),
                    header_text
                )
                .unwrap();
            } else {
                writeln!(&mut output, "=== Session: {} ===", name).unwrap();
            }
            writeln!(&mut output).unwrap();

            for message in session {
                match message {
                    ChatCompletionRequestMessage::User(message) => {
                        if let ChatCompletionRequestUserMessageContent::Text(text) = message.content
                        {
                            render_message_box(
                                &mut output,
                                &text,
                                width,
                                MessageBoxConfig {
                                    label: "User",
                                    color: "\x1b[36m",
                                    max_width_percent: 0.6,
                                    align_right: true,
                                    left_margin: 0,
                                },
                                is_interactive,
                            );
                        }
                    }
                    ChatCompletionRequestMessage::Assistant(message) => {
                        if let Some(content) = &message.content
                            && let ChatCompletionRequestAssistantMessageContent::Text(text) =
                                content
                        {
                            render_message_box(
                                &mut output,
                                text,
                                width,
                                MessageBoxConfig {
                                    label: "Assistant",
                                    color: "\x1b[32m",
                                    max_width_percent: 0.8,
                                    align_right: false,
                                    left_margin: 2,
                                },
                                is_interactive,
                            );
                        }
                    }
                    _ => {}
                }
            }

            if is_interactive {
                let pager = minus::Pager::new();
                pager.set_text(&output).unwrap();
                minus::page_all(pager).unwrap();
            } else {
                print!("{}", output);
            }
        }
        None => {
            println!("Session not found");
        }
    }
}

fn render_message_box(
    output: &mut String,
    text: &str,
    terminal_width: u16,
    config: MessageBoxConfig,
    use_colors: bool,
) {
    use std::fmt::Write as FmtWrite;

    let max_box_width = (terminal_width as f32 * config.max_width_percent) as usize;
    let box_padding = 3;

    let lines: Vec<&str> = text.lines().collect();
    let content_width = lines
        .iter()
        .map(|line| line.len())
        .max()
        .unwrap_or(0)
        .min(max_box_width - box_padding * 2);

    let box_width = content_width + box_padding * 2;
    let left_margin = if config.align_right {
        terminal_width.saturating_sub(box_width as u16 + 2) as usize
    } else {
        config.left_margin
    };

    let label_indent = if config.align_right {
        left_margin + box_width - config.label.len()
    } else {
        left_margin
    };

    if use_colors {
        write!(output, "{}", " ".repeat(label_indent)).unwrap();
        writeln!(output, "{}{}\x1b[0m", config.color, config.label).unwrap();

        // Top border
        write!(output, "{}", " ".repeat(left_margin)).unwrap();
        writeln!(output, "{}╭{}╮\x1b[0m", config.color, "─".repeat(box_width)).unwrap();

        // Content
        for line in lines {
            let display_line = if line.len() > content_width {
                &line[..content_width]
            } else {
                line
            };
            let padding = content_width - display_line.len();

            write!(output, "{}", " ".repeat(left_margin)).unwrap();
            write!(output, "{}│\x1b[0m", config.color).unwrap();
            write!(
                output,
                "{}{}{}",
                " ".repeat(box_padding),
                display_line,
                " ".repeat(padding + box_padding)
            )
            .unwrap();
            writeln!(output, "{}│\x1b[0m", config.color).unwrap();
        }

        // Bottom border
        write!(output, "{}", " ".repeat(left_margin)).unwrap();
        writeln!(output, "{}╰{}╯\x1b[0m", config.color, "─".repeat(box_width)).unwrap();
    } else {
        // Simple text output without colors and box drawing
        writeln!(output, "{}", config.label).unwrap();
        writeln!(output, "{}", "-".repeat(config.label.len())).unwrap();
        for line in lines {
            writeln!(output, "{}", line).unwrap();
        }
    }
    writeln!(output).unwrap();
}
