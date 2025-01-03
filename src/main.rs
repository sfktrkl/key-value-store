use key_value_store::server::Server;

const PEERS: &[(&str, u64)] = &[
    ("localhost:5001", 1),
    ("localhost:5002", 2),
    ("localhost:5003", 3),
];

#[tokio::main]
async fn main() {
    let mut nodes = Vec::new();
    let mut servers = Vec::new();
    for &(address, id) in PEERS {
        let server = Server::new(address, id);
        nodes.push(server.get_node().await);
        servers.push(server);
    }

    for node in &nodes {
        for peer in &nodes {
            if node.id != peer.id {
                let node = node.clone();
                node.add_peer(peer.clone()).await;
            }
        }
    }

    let mut server_handles = Vec::new();
    for server in servers {
        let mut server = server;
        server_handles.push(tokio::spawn(async move {
            server.run().await;
        }));
    }

    for handle in server_handles {
        if let Err(e) = handle.await {
            eprintln!("Server task encountered an error: {:?}", e);
        }
    }
}
