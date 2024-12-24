use crate::serialization::{JsonSerializer, Request, Response, Serializer, SimpleSerializer};
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
            if let Ok((socket, _)) = listener.accept().await {
                println!("Client connected!");

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

            let json = JsonSerializer;
            let simple = SimpleSerializer;
            let input = &buffer[..bytes_read];

            let request = json.deserialize_request(input);
            let (request, is_json) = match request {
                Ok(req) => (Some(req), true),
                Err(_) => match simple.deserialize_request(input) {
                    Ok(req) => (Some(req), false),
                    Err(_) => {
                        let error_response = Response {
                            status: "ERR".to_string(),
                            message: "Invalid request format".to_string(),
                        };
                        let serialized_error = simple.serialize_response(&error_response);
                        let _ = socket.write_all(&serialized_error).await;
                        (None, false)
                    }
                },
            };

            if let Some(request) = request {
                if let Some(response) = Self::process_request(request, &storage).await {
                    let serialized_response = if is_json {
                        json.serialize_response(&response)
                    } else {
                        simple.serialize_response(&response)
                    };

                    if socket.write_all(&serialized_response).await.is_err() {
                        eprintln!("Failed to send response");
                    }
                }
            }
        }
    }

    async fn process_request(request: Request, storage: &Storage) -> Option<Response> {
        match request.command {
            Some(command) => match command.as_str() {
                "put" => {
                    if let (Some(key), Some(value)) = (request.key, request.value) {
                        storage.put(key.clone(), value.clone()).await;
                        Some(Response {
                            status: "OK".to_string(),
                            message: format!("Inserted key '{}' with value '{}'", key, value),
                        })
                    } else {
                        Some(Response {
                            status: "ERR".to_string(),
                            message: "Missing key or value for 'put' command".to_string(),
                        })
                    }
                }
                "get" => {
                    if let Some(key) = request.key {
                        match storage.get(&key).await {
                            Some(value) => Some(Response {
                                status: "OK".to_string(),
                                message: value,
                            }),
                            None => Some(Response {
                                status: "ERR".to_string(),
                                message: format!("Key '{}' not found", key),
                            }),
                        }
                    } else {
                        Some(Response {
                            status: "ERR".to_string(),
                            message: "Missing key for 'get' command".to_string(),
                        })
                    }
                }
                "delete" => {
                    if let Some(key) = request.key {
                        match storage.delete(&key).await {
                            Some(value) => Some(Response {
                                status: "OK".to_string(),
                                message: format!("Deleted key '{}' with value '{}'", key, value),
                            }),
                            None => Some(Response {
                                status: "ERR".to_string(),
                                message: format!("Key '{}' not found", key),
                            }),
                        }
                    } else {
                        Some(Response {
                            status: "ERR".to_string(),
                            message: "Missing key for 'delete' command".to_string(),
                        })
                    }
                }
                _ => Some(Response {
                    status: "ERR".to_string(),
                    message: "Unknown command".to_string(),
                }),
            },
            _ => None,
        }
    }
}
