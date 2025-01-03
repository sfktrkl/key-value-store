use crate::leader::Node;
use crate::serialization::{JsonSerializer, Request, Response, Serializer, SimpleSerializer};
use crate::storage::Storage;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct Server {
    address: String,
    storage: Storage,
    node: Arc<Node>,
}

impl Server {
    pub fn new(address: &str, peers: Vec<u64>, id: u64) -> Self {
        let node: Arc<Node> = Arc::<Node>::new(Node::new(id, peers));
        Self {
            address: address.to_string(),
            storage: Storage::new(),
            node,
        }
    }

    pub async fn run(&mut self) {
        let listener = TcpListener::bind(&self.address)
            .await
            .expect("Failed to bind to address");

        println!("Server running at {}", self.address);

        let node = self.node.clone();
        tokio::spawn(async move {
            node.run().await;
        });

        loop {
            if let Ok((socket, _)) = listener.accept().await {
                println!("Client connected!");
                let node = self.node.clone();
                let storage = self.storage.clone();
                tokio::spawn(async move {
                    Self::handle_client(socket, storage, node).await;
                });
            }
        }
    }

    async fn handle_client(mut socket: TcpStream, storage: Storage, node: Arc<Node>) {
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
                if let Some(response) = Self::process_request(request, &storage, &node).await {
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

    async fn process_request(
        request: Request,
        storage: &Storage,
        _node: &Arc<Node>,
    ) -> Option<Response> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_request_missing_key() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: Some("put".to_string()),
            key: None,
            value: Some("value".to_string()),
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(
            response,
            Some(Response {
                status: "ERR".to_string(),
                message: "Missing key or value for 'put' command".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn test_put_request_missing_value() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: Some("put".to_string()),
            key: Some("key".to_string()),
            value: None,
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(
            response,
            Some(Response {
                status: "ERR".to_string(),
                message: "Missing key or value for 'put' command".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn test_get_request_missing_key() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: Some("get".to_string()),
            key: None,
            value: None,
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(
            response,
            Some(Response {
                status: "ERR".to_string(),
                message: "Missing key for 'get' command".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn test_delete_request_missing_key() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: Some("delete".to_string()),
            key: None,
            value: None,
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(
            response,
            Some(Response {
                status: "ERR".to_string(),
                message: "Missing key for 'delete' command".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn test_unknown_command() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: Some("unknown".to_string()),
            key: Some("key".to_string()),
            value: Some("value".to_string()),
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(
            response,
            Some(Response {
                status: "ERR".to_string(),
                message: "Unknown command".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn test_invalid_request() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: None,
            key: Some("key".to_string()),
            value: Some("value".to_string()),
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(response, None);
    }

    #[tokio::test]
    async fn test_empty_request() {
        let storage = Storage::new();
        let node = Arc::new(Node::new(1, vec![]));

        let request = Request {
            command: None,
            key: None,
            value: None,
        };
        let response = Server::process_request(request, &storage, &node).await;
        assert_eq!(response, None);
    }
}
