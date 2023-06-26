mod parser;
mod stratify;
pub mod ast;
pub mod context;
use std::fs::read_to_string;
use parser::parse_program;
use context::Context;

pub fn parse(source: &str) -> Context {
    let input = read_to_string(source).unwrap();
    let (remain, program) = parse_program(&input).unwrap();
    if !remain.is_empty() {
        panic!("Parsing error:\nparsing remain: \"{}\"", remain);
    }
    let context = Context::new(&program);
    context
}
