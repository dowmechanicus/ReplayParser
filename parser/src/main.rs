use std::env;

use parser_lib;

fn main() {
    if let Some(arg) = env::args().nth(1) {
        match parser_lib::parse_file(arg) {
            Some(json) => println!("{}", json),
            None => println!("File could not be parsed"),
        }
    } else {
        println!("No input file specified");
    }
}
