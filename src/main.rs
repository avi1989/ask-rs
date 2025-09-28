use clap::Parser;

mod llms;

#[derive(Parser)]
struct Cli {
    question: Vec<String>,
}

#[tokio::main]
async fn main() {

    let cli = Cli::parse();

    let question = cli.question.join(" ");
    let answer = llms::ask_question(&question).await;
    println!("Answer: {}", answer);

    println!("You asked: {}", cli.question.join(" "));
    
}
