mod server;
mod storage;

use server::Server;

const SERVER_ADDRESS: &str = "localhost:5000";

#[tokio::main]
async fn main() {
    let mut server = Server::new(SERVER_ADDRESS);
    server.run().await;
}
