mod cli;
mod storage;

use cli::CommandLineInterface;
use storage::Storage;

fn main() {
    let storage = Storage::new();
    let mut cli = CommandLineInterface::new(storage);
    cli.run();
}
