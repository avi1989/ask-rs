use clap::Parser;

mod llms;
mod tools;
mod shell;

#[derive(Parser)]
struct Cli {
    question: Vec<String>,
}

#[tokio::main]
async fn main() {

    let cli = Cli::parse();

    let question = cli.question.join(" ");
    let answer = llms::ask_question(&question).await.unwrap();
    markterm::render_text_to_stdout(&answer, None, markterm::ColorChoice::Auto).unwrap()
}
