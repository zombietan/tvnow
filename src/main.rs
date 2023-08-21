use std::env;
use std::io::{stderr, stdout};
use tvnow::Cli;

fn main() -> ! {
    Cli::new(stdout(), stderr()).execute(env::args()).exit()
}
