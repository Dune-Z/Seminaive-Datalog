mod syntax;
mod engine;
use clap::Parser;
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(author)]
struct Args {
    #[arg(long)]
    source: String,
    #[arg(long, default_value = "false")]
    verbose: bool,
    #[arg(long, default_value = "false")]
    bench: bool,
}

fn main() {
    let cli = Args::parse();
    let now = Instant::now();
    engine::run(&cli.source[..], cli.verbose);
    let elapsed = now.elapsed();
    if cli.bench {
        println!("{}.{:03}s", elapsed.as_secs(), elapsed.subsec_millis());
    }
}
