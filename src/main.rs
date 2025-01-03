use key_value_store::server::Server;

const PEERS: &[(&str, u64)] = &[
    ("localhost:5001", 1),
    ("localhost:5002", 2),
    ("localhost:5003", 3),
];

#[tokio::main]
async fn main() {
    let mut servers = Vec::new();

    for &(address, id) in PEERS {
        let peers: Vec<u64> = PEERS
            .iter()
            .filter(|&&(_, peer_id)| peer_id != id)
            .map(|&(_, peer_id)| peer_id)
            .collect();

        let server = Server::new(address, peers, id);
        servers.push(server);
    }

    let mut server_handles = Vec::new();
    for mut server in servers {
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
