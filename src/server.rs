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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::future::Future;
    use std::sync::Arc;
    use std::time::Duration;

    const DURATION: core::time::Duration = Duration::from_millis(100);

    async fn run_server(address: &str) {
        let mut server = Server::new(address);
        tokio::spawn(async move {
            server.run().await;
        });
        tokio::time::sleep(DURATION).await;
    }

    async fn client_connect(address: &str) -> TcpStream {
        TcpStream::connect(address)
            .await
            .expect("Failed to connect to server")
    }

    async fn client_task(
        stream: &mut TcpStream,
        request: Request,
        serializer: &dyn Serializer,
    ) -> Result<Response, String> {
        let request_bytes = serializer.serialize_request(&request);

        stream
            .write_all(&request_bytes)
            .await
            .expect("Failed to send request");

        let mut response_buffer = vec![0; 1024];
        let bytes_read = stream
            .read(&mut response_buffer)
            .await
            .expect("Failed to read response");

        serializer.deserialize_response(&response_buffer[..bytes_read])
    }

    async fn client_execute<F, Fut>(address: &str, f: F)
    where
        F: FnOnce(TcpStream) -> Fut,
        Fut: Future<Output = ()>,
    {
        f(client_connect(&address).await).await;
        tokio::time::sleep(DURATION).await;
    }

    #[rstest]
    #[case(Box::new(JsonSerializer), 5001)]
    #[case(Box::new(SimpleSerializer), 5002)]
    #[tokio::test]
    async fn test_single_client_access(#[case] serializer: Box<dyn Serializer>, #[case] port: u16) {
        let address = format!("localhost:{}", port);
        run_server(&address).await;

        let serializer = serializer.as_ref();
        let put_request = Request {
            command: Some("put".to_string()),
            key: Some("foo".to_string()),
            value: Some("bar".to_string()),
        };
        let get_request = Request {
            command: Some("get".to_string()),
            key: Some("foo".to_string()),
            value: None,
        };
        let delete_request = Request {
            command: Some("delete".to_string()),
            key: Some("foo".to_string()),
            value: None,
        };

        client_execute(&address, |mut stream| async move {
            let response = client_task(&mut stream, put_request, serializer).await;
            let expected = Response {
                status: "OK".to_string(),
                message: "Inserted key 'foo' with value 'bar'".to_string(),
            };
            assert_eq!(response.unwrap(), expected);

            let response = client_task(&mut stream, get_request, serializer).await;
            let expected = Response {
                status: "OK".to_string(),
                message: "bar".to_string(),
            };
            assert_eq!(response.unwrap(), expected);

            let response = client_task(&mut stream, delete_request, serializer).await;
            let expected = Response {
                status: "OK".to_string(),
                message: "Deleted key 'foo' with value 'bar'".to_string(),
            };
            assert_eq!(response.unwrap(), expected);
        })
        .await;
    }

    #[rstest]
    #[case(Arc::new(JsonSerializer), 5003)]
    #[case(Arc::new(SimpleSerializer), 5004)]
    #[tokio::test]
    async fn test_multiple_client_access(
        #[case] serializer: Arc<dyn Serializer + Send + Sync>,
        #[case] port: u16,
    ) {
        let address = format!("localhost:{}", port);
        run_server(&address).await;

        let client_tasks: Vec<_> = (0..5)
            .map(|i| {
                let address = address.clone();
                let serializer = Arc::clone(&serializer);
                tokio::spawn(async move {
                    println!("Task {} started on thread", i);

                    let serializer = serializer.as_ref();
                    let put_request = Request {
                        command: Some("put".to_string()),
                        key: Some(format!("key{}", i)),
                        value: Some(format!("value{}", i)),
                    };
                    let get_request = Request {
                        command: Some("get".to_string()),
                        key: Some(format!("key{}", i)),
                        value: None,
                    };
                    let delete_request = Request {
                        command: Some("delete".to_string()),
                        key: Some(format!("key{}", i)),
                        value: None,
                    };

                    client_execute(&address, move |mut stream| async move {
                        let response = client_task(&mut stream, put_request, serializer).await;
                        let expected = Response {
                            status: "OK".to_string(),
                            message: format!("Inserted key 'key{}' with value 'value{}'", i, i),
                        };
                        assert_eq!(response.unwrap(), expected);

                        let response = client_task(&mut stream, get_request, serializer).await;
                        let expected = Response {
                            status: "OK".to_string(),
                            message: format!("value{}", i),
                        };
                        assert_eq!(response.unwrap(), expected);

                        let response = client_task(&mut stream, delete_request, serializer).await;
                        let expected = Response {
                            status: "OK".to_string(),
                            message: format!("Deleted key 'key{}' with value 'value{}'", i, i),
                        };
                        assert_eq!(response.unwrap(), expected);
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
