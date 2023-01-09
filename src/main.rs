mod basic_parser;
use basic_parser::*;
mod tokens;
mod ast;
use ast::*;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut iter = args[1].chars().peekable();
    println!("{:?}", is_some::<Sudo>(&mut iter));
    println!("---");
    println!("{}", iter.collect::<String>());
}
