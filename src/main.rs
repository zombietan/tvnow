use std::env;
use std::io::{stderr, stdout};
use tvnow::Cli;

#[async_std::main]
async fn main() -> ! {
    Cli::new(stdout(), stderr())
        .execute(env::args())
        .await
        .exit()
}
