use crate::storage::Storage;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct Server {
    address: String,
    storage: Storage,
}

impl Server {
    pub fn new(address: &str) -> Self {
        Self {
            address: address.to_string(),
            storage: Storage::new(),
        }
    }

    pub async fn run(&mut self) {
        let listener = TcpListener::bind(&self.address)
            .await
            .expect("Failed to bind to address");

        println!("Server running at {}", self.address);

        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                println!("Client connected!");

                let welcome_message = "Welcome to the Key-Value Store!\n";
                if socket.write_all(welcome_message.as_bytes()).await.is_err() {
                    eprintln!("Failed to send welcome message");
                    continue;
                }

                Self::handle_client(socket, &mut self.storage).await;
            }
        }
    }

    async fn handle_client(mut socket: TcpStream, storage: &mut Storage) {
        let mut buffer = [0u8; 1024];

        while let Ok(bytes_read) = socket.read(&mut buffer).await {
            if bytes_read == 0 {
                println!("Client disconnected!");
                break;
            }

            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
            let response = Self::process_request(&request, storage);
            if socket.write_all(response.as_bytes()).await.is_err() {
                eprintln!("Failed to send response");
                break;
            }
        }
    }

    fn process_request(request: &str, storage: &mut Storage) -> String {
        let parts: Vec<&str> = request.trim().splitn(3, ' ').collect();

        match parts.as_slice() {
            ["put", key, value] => {
                storage.put(key.to_string(), value.to_string());
                format!("OK: Inserted key '{}' with value '{}'\n", key, value)
            }
            ["get", key] => match storage.get(key) {
                Some(value) => format!("OK: {}\n", value),
                None => format!("ERR: Key '{}' not found\n", key),
            },
            ["delete", key] => match storage.delete(key) {
                Some(value) => format!("OK: Deleted key '{}' with value '{}'\n", key, value),
                None => format!("ERR: Key '{}' not found\n", key),
            },
            _ => "ERR: Unknown command\n".to_string(),
        }
    }
}

