use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};
use tokio::time;

use crate::log::LogEntry;

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Leader,
    Follower,
    Candidate,
}

pub struct Node {
    pub id: u64,
    term: Arc<Mutex<u64>>,
    pub role: Arc<RwLock<Role>>,
    peers: RwLock<Vec<Arc<Node>>>,
    last_heartbeat: AtomicU64,
    log: Mutex<Vec<LogEntry>>,
    commit_index: AtomicU64,
    last_applied: AtomicU64,
}

impl Node {
    pub fn new(id: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            id,
            term: Arc::new(Mutex::new(0)),
            role: Arc::new(RwLock::new(Role::Follower)),
            peers: RwLock::new(Vec::new()),
            last_heartbeat: AtomicU64::new(now - 5000),
            log: Mutex::new(Vec::new()),
            commit_index: AtomicU64::new(0),
            last_applied: AtomicU64::new(0),
        }
    }

    pub async fn add_peer(self: Arc<Self>, peer: Arc<Node>) {
        let mut peers = self.peers.write().await;
        peers.push(peer);
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

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            if now < self.last_heartbeat.load(Ordering::Relaxed) + 1500 {
                println!("Node {}: Recent heartbeat received.", self.id);
                continue;
            }

            {
                if *self.role.read().await == Role::Follower {
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

        // Increment the term before requesting votes
        {
            let mut term = self.term.lock().await;
            *term += 1;
        }

        println!(
            "Node {}: Became Candidate for term {}.",
            self.id,
            *self.term.lock().await
        );

        // Request votes from peers
        let mut vote_tasks = vec![];
        for peer in self.peers.read().await.iter() {
            let id = self.id;
            let peer = peer.clone();
            let term = *self.term.lock().await;
            let task = tokio::spawn(async move { peer.request_vote(id, term).await });
            vote_tasks.push(task);
        }

        // Await vote results and count them
        let mut votes = 1;
        for task in vote_tasks {
            if let Ok(vote_granted) = task.await {
                if vote_granted {
                    votes += 1;
                }
            }
        }

        // If this node received enough votes, it becomes the leader
        if votes > (self.peers.read().await.len() + 1) / 2 {
            println!("Node {}: Won election and became Leader.", self.id);
            let mut role = self.role.write().await;
            *role = Role::Leader;
        } else {
            println!("Node {}: Lost election.", self.id);
            let mut role = self.role.write().await;
            *role = Role::Follower;
        }
    }

    async fn request_vote(&self, candidate_id: u64, candidate_term: u64) -> bool {
        let mut term = self.term.lock().await;
        let role = self.role.read().await;

        if *role == Role::Leader {
            println!(
                "Node {}: Cannot vote, already a leader in term {}.",
                self.id, *term
            );
            return false;
        }

        if *term <= candidate_term {
            println!(
                "Node {}: Granting vote for Node {} in term {}.",
                self.id, candidate_id, candidate_term
            );
            *term = term.max(candidate_term);
            return true;
        }

        println!(
            "Node {}: Denying vote for Node {} in term {}.",
            self.id, candidate_id, *term
        );
        false
    }

    async fn send_heartbeat(self: &Arc<Self>) {
        println!("Node {}: Sending heartbeat.", self.id);
        for peer in self.peers.read().await.iter() {
            let peer = peer.clone();
            let self_term = *self.term.lock().await;
            tokio::spawn(async move {
                peer.receive_heartbeat(self_term).await;
            });
        }
    }

    async fn receive_heartbeat(&self, leader_term: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        self.last_heartbeat.store(now, Ordering::Relaxed);

        let mut term = self.term.lock().await;
        if leader_term > *term {
            *term = leader_term;
        }
    }

    pub async fn handle_client_request(self: Arc<Self>, command: String) {
        // Only the leader should handle client requests
        {
            let role = self.role.read().await;
            if *role != Role::Leader {
                println!(
                    "Node {}: Not the leader, cannot handle client request.",
                    self.id
                );
                return;
            }
        }

        // Create a new log entry
        let new_log_entry = {
            let log = self.log.lock().await;
            let term = *self.term.lock().await;
            let index = log.len() as u64 + 1; // Log index starts at 1

            LogEntry {
                term,
                index,
                command: command.clone(),
            }
        };

        // Append the new entry to the leader's log
        {
            let mut log = self.log.lock().await;
            log.push(new_log_entry.clone());
        }

        println!(
            "Node {}: Appended log entry {:?} to its log.",
            self.id, new_log_entry
        );

        // Start replication to followers
        self.clone().send_append_entries().await;

        // Apply the command to the local state machine once committed
        self.clone().apply_committed_logs().await;
    }

    pub async fn send_append_entries(self: Arc<Self>) {
        let log = self.log.lock().await;
        let term = *self.term.lock().await;

        for peer in self.peers.read().await.iter() {
            let id = self.id;
            let peer = peer.clone();
            let peer_id = peer.id;
            let entries_to_replicate = log.clone(); // Clone the log for sending
            tokio::spawn(async move {
                let success = peer
                    .receive_append_entries(term, entries_to_replicate)
                    .await;

                if success {
                    println!("Node {}: AppendEntries successful for peer {}", id, peer_id);
                } else {
                    println!("Node {}: AppendEntries failed for peer {}", id, peer_id);
                }
            });
        }
    }

    pub async fn receive_append_entries(
        self: Arc<Self>,
        leader_term: u64,
        entries: Vec<LogEntry>,
    ) -> bool {
        let mut term = self.term.lock().await;

        if leader_term < *term {
            return false; // Reject entries if leader's term is outdated
        }

        // Update term if necessary
        if leader_term > *term {
            *term = leader_term;
            let mut role = self.role.write().await;
            *role = Role::Follower; // Become a follower if term is higher
        }

        // Append new log entries
        let mut log = self.log.lock().await;
        log.extend(entries);

        println!("Node {}: Appended entries from leader.", self.id);
        true
    }

    pub async fn apply_committed_logs(self: Arc<Self>) {
        let mut log = self.log.lock().await;

        for entry in log.iter() {
            if entry.index > self.last_applied.load(Ordering::Relaxed) {
                println!("Node {}: Applying log entry {:?}", self.id, entry);

                // Parse and apply the command to the local storage
                let parts: Vec<&str> = entry.command.split_whitespace().collect();
                if parts.len() >= 3 {
                    let cmd = parts[0];
                    let key = parts[1].to_string();
                    let value = parts[2].to_string();

                    /*
                    match cmd {
                        "put" => {
                            self.storage.put(key.clone(), value.clone()).await;
                            println!(
                                "Node {}: Applied 'put' command: key = {}, value = {}",
                                self.id, key, value
                            );
                        }
                        "delete" => {
                            self.storage.delete(&key).await;
                            println!("Node {}: Applied 'delete' command: key = {}", self.id, key);
                        }
                        _ => {
                            println!("Node {}: Unknown command in log entry", self.id);
                        }
                    }
                    */
                }

                self.last_applied.store(entry.index, Ordering::Relaxed); // Update applied index
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_node_initialization() {
        let node = Node::new(1);

        assert_eq!(node.id, 1);
        assert_eq!(*node.role.read().await, Role::Follower);
        assert_eq!(*node.term.lock().await, 0);
        assert_eq!(node.peers.read().await.len(), 0);
        assert!(node.last_heartbeat.load(Ordering::Relaxed) > 0);
    }

    #[tokio::test]
    async fn test_adding_peers() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        node1.clone().add_peer(node2.clone()).await;

        assert_eq!(node1.peers.read().await.len(), 1);
        assert_eq!(node1.peers.read().await[0].id, 2);
    }

    #[tokio::test]
    async fn test_heartbeat_handling() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        node1.clone().add_peer(node2.clone()).await;

        let heartbeat = node2.last_heartbeat.load(Ordering::Relaxed);

        node1.clone().send_heartbeat().await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(node2.last_heartbeat.load(Ordering::Relaxed) > heartbeat);
    }

    #[tokio::test]
    async fn test_vote_request_granted() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        assert!(node2.request_vote(node1.id, 1).await);
        assert_eq!(*node2.term.lock().await, 1);
    }

    #[tokio::test]
    async fn test_vote_request_denied() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        {
            let mut term = node2.term.lock().await;
            *term = 2;
        }

        assert!(!node2.request_vote(node1.id, 1).await);
    }

    #[tokio::test]
    async fn test_vote_request_denied_leader() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        {
            let mut role = node1.role.write().await;
            *role = Role::Leader;
        }

        assert!(!node1.request_vote(node2.id, 1).await);
    }

    #[tokio::test]
    async fn test_election() {
        let node = Arc::new(Node::new(1));

        tokio::spawn(node.clone().start_election_timer());

        // Simulate election timeout
        tokio::time::sleep(Duration::from_millis(2100)).await;

        assert_eq!(*node.role.read().await, Role::Leader);
        assert!(*node.term.lock().await > 0);
    }

    #[tokio::test]
    async fn test_election_with_multiple_nodes() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        node1.clone().add_peer(node2.clone()).await;
        node2.clone().add_peer(node1.clone()).await;

        tokio::spawn(node1.clone().start_election_timer());
        tokio::spawn(node2.clone().start_election_timer());

        // Simulate election timeout
        tokio::time::sleep(Duration::from_millis(2100)).await;

        let role1 = node1.role.read().await;
        let role2 = node2.role.read().await;

        assert!(matches!(*role1, Role::Leader) || matches!(*role2, Role::Leader));
        assert!(matches!(*role1, Role::Follower) || matches!(*role2, Role::Follower));
    }

    #[tokio::test]
    async fn test_election_hearbeat_prevents_election() {
        let node1 = Arc::new(Node::new(1));
        let node2 = Arc::new(Node::new(2));

        node1.clone().add_peer(node2.clone()).await;
        node2.clone().add_peer(node1.clone()).await;

        tokio::spawn(node1.clone().start_election_timer());
        tokio::spawn(node2.clone().start_election_timer());

        // Simulate election timeout
        tokio::time::sleep(Duration::from_millis(500)).await;

        let heartbeat = node2.last_heartbeat.load(Ordering::Relaxed);

        node1.send_heartbeat().await;

        tokio::time::sleep(Duration::from_millis(2100)).await;

        assert_eq!(*node2.role.read().await, Role::Follower);
        assert!(node2.last_heartbeat.load(Ordering::Relaxed) > heartbeat);
    }
}
