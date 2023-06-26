mod syntax;
mod engine;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author)]
struct Args {
    #[arg(long)]
    source: String,
    #[arg(long, default_value = "false")]
    verbose: bool,
}

fn main() {
    let cli = Args::parse();
    engine::run(&cli.source[..], cli.verbose);
}
