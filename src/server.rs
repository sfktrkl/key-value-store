use crate::serialization::{Request, Response};
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

                let storage = self.storage.clone();
                tokio::spawn(async move {
                    Self::handle_client(socket, storage).await;
                });
            }
        }
    }

    async fn handle_client(mut socket: TcpStream, storage: Storage) {
        let mut buffer = [0u8; 1024];

        while let Ok(bytes_read) = socket.read(&mut buffer).await {
            if bytes_read == 0 {
                println!("Client disconnected!");
                break;
            }

            let request: Request = match serde_json::from_slice(&buffer[..bytes_read]) {
                Ok(req) => req,
                Err(_) => {
                    let error_response = Response {
                        status: "ERR".to_string(),
                        message: "Invalid request format".to_string(),
                    };
                    let response_bytes = serde_json::to_vec(&error_response).unwrap();
                    let _ = socket.write_all(&response_bytes).await;
                    continue;
                }
            };

            let response = Self::process_request(request, &storage).await;
            let response_bytes = serde_json::to_vec(&response).unwrap();
            if socket.write_all(&response_bytes).await.is_err() {
                eprintln!("Failed to send response");
            }
        }
    }

    async fn process_request(request: Request, storage: &Storage) -> Response {
        match request.command.as_str() {
            "put" => {
                if let (Some(key), Some(value)) = (request.key, request.value) {
                    storage.put(key.clone(), value.clone()).await;
                    Response {
                        status: "OK".to_string(),
                        message: format!("Inserted key '{}' with value '{}'", key, value),
                    }
                } else {
                    Response {
                        status: "ERR".to_string(),
                        message: "Missing key or value for 'put' command".to_string(),
                    }
                }
            }
            "get" => {
                if let Some(key) = request.key {
                    match storage.get(&key).await {
                        Some(value) => Response {
                            status: "OK".to_string(),
                            message: value,
                        },
                        None => Response {
                            status: "ERR".to_string(),
                            message: format!("Key '{}' not found", key),
                        },
                    }
                } else {
                    Response {
                        status: "ERR".to_string(),
                        message: "Missing key for 'get' command".to_string(),
                    }
                }
            }
            "delete" => {
                if let Some(key) = request.key {
                    match storage.delete(&key).await {
                        Some(value) => Response {
                            status: "OK".to_string(),
                            message: format!("Deleted key '{}' with value '{}'", key, value),
                        },
                        None => Response {
                            status: "ERR".to_string(),
                            message: format!("Key '{}' not found", key),
                        },
                    }
                } else {
                    Response {
                        status: "ERR".to_string(),
                        message: "Missing key for 'delete' command".to_string(),
                    }
                }
            }
            _ => Response {
                status: "ERR".to_string(),
                message: "Unknown command".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::time::Duration;

    const DURATION: core::time::Duration = Duration::from_millis(100);

    async fn run_server(address: &str) {
        let mut server = Server::new(address);
        tokio::spawn(async move {
            server.run().await;
        });
        tokio::time::sleep(DURATION).await;
    }

    async fn client_connect(address: &str) -> (TcpStream, String) {
        let mut stream = TcpStream::connect(address)
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

    async fn client_task(stream: &mut TcpStream, request: Request) -> String {
        let request_str = serde_json::to_string(&request).expect("Failed to serialize request");

        stream
            .write_all(request_str.as_bytes())
            .await
            .expect("Failed to write request");

        let mut response = vec![0; 1024];
        let bytes_read = stream
            .read(&mut response)
            .await
            .expect("Failed to read response");

        String::from_utf8_lossy(&response[..bytes_read]).to_string()
    }

    async fn client_execute<F, Fut>(address: &str, f: F)
    where
        F: FnOnce(TcpStream) -> Fut,
        Fut: Future<Output = ()>,
    {
        let (stream, response) = client_connect(address).await;
        assert_eq!(response, "Welcome to the Key-Value Store!\n");

        f(stream).await;

        tokio::time::sleep(DURATION).await;
    }

    #[tokio::test]
    async fn test_single_client_access() {
        let address = "localhost:5000";
        run_server(&address).await;

        client_execute(&address, |mut stream| async move {
            let request = Request {
                command: "put".to_string(),
                key: Some("foo".to_string()),
                value: Some("bar".to_string()),
            };
            let response = client_task(&mut stream, request).await;
            assert_eq!(
                response,
                "{\"status\":\"OK\",\"message\":\"Inserted key 'foo' with value 'bar'\"}"
            );

            let request = Request {
                command: "get".to_string(),
                key: Some("foo".to_string()),
                value: None,
            };
            let response = client_task(&mut stream, request).await;
            assert_eq!(response, "{\"status\":\"OK\",\"message\":\"bar\"}");

            let request = Request {
                command: "delete".to_string(),
                key: Some("foo".to_string()),
                value: None,
            };
            let response = client_task(&mut stream, request).await;
            assert_eq!(
                response,
                "{\"status\":\"OK\",\"message\":\"Deleted key 'foo' with value 'bar'\"}"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn test_multiple_client_access() {
        let address = "localhost:5001";
        run_server(&address).await;

        let client_tasks: Vec<_> = (0..5)
            .map(|i| {
                tokio::spawn(async move {
                    println!("Task {} started on thread", i);
                    let put_request = Request {
                        command: "put".to_string(),
                        key: Some(format!("key{}", i)),
                        value: Some(format!("value{}", i)),
                    };
                    let get_request = Request {
                        command: "get".to_string(),
                        key: Some(format!("key{}", i)),
                        value: None,
                    };
                    let delete_request = Request {
                        command: "delete".to_string(),
                        key: Some(format!("key{}", i)),
                        value: None,
                    };
                    client_execute(&address, move |mut stream| async move {
                        let response = client_task(&mut stream, put_request).await;
                        assert_eq!(
                            response,
                            format!("{{\"status\":\"OK\",\"message\":\"Inserted key 'key{}' with value 'value{}'\"}}", i, i)
                        );

                        let response = client_task(&mut stream, get_request).await;
                        assert_eq!(
                            response,
                            format!("{{\"status\":\"OK\",\"message\":\"value{}\"}}", i)
                        );

                        let response = client_task(&mut stream, delete_request).await;
                        assert_eq!(
                            response,
                            format!("{{\"status\":\"OK\",\"message\":\"Deleted key 'key{}' with value 'value{}'\"}}", i, i)
                        );
                    })
                    .await
                })
            })
            .collect();

        for task in client_tasks {
            let result = task.await;
            if let Err(e) = result {
                eprintln!("Task failed with error: {:?}", e);
                assert!(false, "Some tasks failed");
            }
        }
    }
}
