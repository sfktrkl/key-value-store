use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time;

#[derive(Debug, Clone, PartialEq)]
enum Role {
    Leader,
    Follower,
    Candidate,
}

pub struct Node {
    id: u64,
    peers: Vec<u64>,
    role: Arc<RwLock<Role>>,
    term: Arc<Mutex<u64>>,
    votes_received: Arc<Mutex<u64>>,
}

impl Node {
    pub fn new(id: u64, peers: Vec<u64>) -> Self {
        Self {
            id,
            peers,
            role: Arc::new(RwLock::new(Role::Follower)),
            term: Arc::new(Mutex::new(0)),
            votes_received: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn run(self: Arc<Self>) {
        let node = self.clone();
        tokio::spawn(async move {
            node.start_election_timer().await;
        });
    }

    async fn start_election_timer(self: Arc<Self>) {
        loop {
            {
                let role = self.role.read().await;
                if *role == Role::Leader {
                    // Leader sends heartbeats
                    self.send_heartbeat().await;
                    time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            }

            let election_timeout = Duration::from_millis(1500 + rand::random::<u64>() % 500);
            time::sleep(election_timeout).await;

            {
                let role = self.role.read().await;
                if *role == Role::Follower {
                    println!("Node {}: Timeout! Starting election.", self.id);
                    self.start_election().await;
                }
            }
        }
    }

    async fn start_election(self: Arc<Self>) {
        *self.role.write().await = Role::Candidate;
        *self.term.lock().await += 1;
        *self.votes_received.lock().await = 1; // Vote for self

        println!(
            "Node {}: Became Candidate for term {}.",
            self.id,
            *self.term.lock().await
        );

        // Request votes from peers
        for peer in &self.peers {
            let node = self.clone();
            let peer_id = *peer;
            tokio::spawn(async move {
                if node.request_vote(peer_id).await {
                    *node.votes_received.lock().await += 1;
                }
            });
        }

        // Wait for votes or timeout
        time::sleep(Duration::from_millis(1000)).await;

        let votes = *self.votes_received.lock().await;
        if votes as usize > (self.peers.len() + 1) / 2 {
            println!("Node {}: Won election and became Leader.", self.id);
            *self.role.write().await = Role::Leader;
        } else {
            println!("Node {}: Lost election.", self.id);
            *self.role.write().await = Role::Follower;
        }
    }

    async fn request_vote(&self, peer_id: u64) -> bool {
        // Simulate requesting a vote from a peer
        println!("Node {}: Requesting vote from Node {}.", self.id, peer_id);
        true // Simulating a granted vote for now
    }

    async fn send_heartbeat(&self) {
        println!("Node {}: Sending heartbeat.", self.id);
        for peer in &self.peers {
            println!("Node {}: Heartbeat sent to Node {}.", self.id, peer);
        }
    }
}
