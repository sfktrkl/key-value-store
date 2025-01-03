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
                    self.clone().start_election().await;
                }
            }
        }
    }

    async fn start_election(self: Arc<Self>) {
        {
            let mut role = self.role.write().await;
            if *role != Role::Follower {
                return;
            }
            *role = Role::Candidate;
        }

        {
            let mut term = self.term.lock().await;
            *term += 1;
        }

        {
            let mut votes_received = self.votes_received.lock().await;
            *votes_received = 1; // Vote for self
        }

        println!(
            "Node {}: Became Candidate for term {}.",
            self.id,
            *self.term.lock().await
        );
        // Request votes from peers
        let mut votes = 1; // Start with self-vote
        let mut vote_tasks = Vec::new();

        for &peer_id in &self.peers {
            let node = self.clone();
            let task = tokio::spawn(async move { node.request_vote(peer_id).await });
            vote_tasks.push(task);
        }

        for task in vote_tasks {
            if let Ok(vote_granted) = task.await {
                if vote_granted {
                    votes += 1;
                }
            }
        }

        if votes > (self.peers.len() + 1) / 2 {
            println!("Node {}: Won election and became Leader.", self.id);
            let mut role = self.role.write().await;
            *role = Role::Leader;
        } else {
            println!("Node {}: Lost election.", self.id);
            let mut role = self.role.write().await;
            *role = Role::Follower;
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
