use key_value_store::server::Server;

const SERVER_ADDRESS: &str = "localhost:5000";
const NODE_ID: u64 = 1; // Unique ID for this node
const PEERS: &[u64] = &[2, 3]; // IDs of other nodes in the cluster

#[tokio::main]
async fn main() {
    let mut server = Server::new(SERVER_ADDRESS, PEERS.to_vec(), NODE_ID);
    server.run().await;
}
