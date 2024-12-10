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

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::time::Duration;

    const SERVER_ADDRESS: &str = "localhost:5000";
    const DURATION: core::time::Duration = Duration::from_millis(100);

    async fn run_server() {
        let mut server = Server::new(SERVER_ADDRESS);
        tokio::spawn(async move {
            server.run().await;
        });
        tokio::time::sleep(DURATION).await;
    }

    async fn client_connect() -> (TcpStream, String) {
        let mut stream = TcpStream::connect(SERVER_ADDRESS)
            .await
            .expect("Failed to connect to server");

        let mut response = vec![0; 1024];
        let bytes_read = stream
            .read(&mut response)
            .await
            .expect("Failed to read response");

        (
            stream,
            String::from_utf8_lossy(&response[..bytes_read]).to_string(),
        )
    }

    async fn client_task(stream: &mut TcpStream, request: &str) -> String {
        stream
            .write_all(request.as_bytes())
            .await
            .expect("Failed to write request");

        let mut response = vec![0; 1024];
        let bytes_read = stream
            .read(&mut response)
            .await
            .expect("Failed to read response");

        String::from_utf8_lossy(&response[..bytes_read]).to_string()
    }

    async fn client_execute<F, Fut>(f: F)
    where
        F: FnOnce(TcpStream) -> Fut,
        Fut: Future<Output = ()>,
    {
        let (stream, response) = client_connect().await;
        assert_eq!(response, "Welcome to the Key-Value Store!\n");

        f(stream).await;

        tokio::time::sleep(DURATION).await;
    }

    #[tokio::test]
    async fn test_single_client_access() {
        run_server().await;

        client_execute(|mut stream| async move {
            let response = client_task(&mut stream, "put foo bar\n").await;
            assert_eq!(response, "OK: Inserted key 'foo' with value 'bar'\n");

            let response = client_task(&mut stream, "get foo\n").await;
            assert_eq!(response, "OK: bar\n");

            let response = client_task(&mut stream, "delete foo\n").await;
            assert_eq!(response, "OK: Deleted key 'foo' with value 'bar'\n");
        })
        .await;
    }
}
