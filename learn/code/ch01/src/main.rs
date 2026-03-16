// Chapter 1: Hello CLI — Code snapshot

use std::io::{self, BufRead, Write};

fn main() {
    println!("Welcome to the CLI Agent!");
    println!("Type 'quit' to exit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("> ");
        stdout.flush().unwrap();

        let mut line = String::new();
        stdin.lock().read_line(&mut line).unwrap();
        let input = line.trim();

        if input == "quit" {
            println!("Goodbye!");
            break;
        }

        // TODO: Process user input
        println!("You said: {input}");
    }
}
