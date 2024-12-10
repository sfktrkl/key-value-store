use crate::storage::Storage;

pub struct CommandLineInterface {
    pub storage: Storage,
}

impl CommandLineInterface {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn run(&mut self) {
        use std::io::{self, Write};

        println!("Welcome to the Key-Value Store!");
        loop {
            print!("> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                eprintln!("Failed to read input.");
                continue;
            }

            let parts: Vec<&str> = input.trim().splitn(3, ' ').collect();
            match parts.as_slice() {
                ["put", key, value] => {
                    self.storage.put(key.to_string(), value.to_string());
                    println!("Inserted key '{}' with value '{}'.", key, value);
                }
                ["get", key] => match self.storage.get(key) {
                    Some(value) => println!("Value: {}", value),
                    None => println!("Key '{}' not found.", key),
                },
                ["delete", key] => match self.storage.delete(key) {
                    Some(value) => println!("Deleted key '{}' with value '{}'.", key, value),
                    None => println!("Key '{}' not found.", key),
                },
                ["exit"] => {
                    println!("Exiting...");
                    break;
                }
                _ => println!("Unknown command. Available commands: put, get, delete, exit."),
            }
        }
    }
}
