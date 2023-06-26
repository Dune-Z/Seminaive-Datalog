use super::syntax::{context, ast};
use super::syntax::parse;
use colored::Colorize;
mod runtime;
mod analysis;
use runtime::Runtime;

pub fn run(source_path: &str, verbose: bool) {
    let runtime = Runtime::new(source_path, verbose);
    match runtime {
        Ok(runtime) => {
            let _result = runtime.eval();
        },
        Err(error) => {
            println!("{}: {}", "ERROR".red(), error);
        }
    }
}