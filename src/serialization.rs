use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Request {
    pub command: Option<String>,
    pub key: Option<String>,
    pub value: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Response {
    pub status: String,
    pub message: String,
}

pub trait Serializer: Send + Sync {
    fn serialize_request(&self, request: &Request) -> Vec<u8>;
    fn deserialize_request(&self, bytes: &[u8]) -> Result<Request, String>;
    fn serialize_response(&self, response: &Response) -> Vec<u8>;
    fn deserialize_response(&self, bytes: &[u8]) -> Result<Response, String>;
}

pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    fn serialize_request(&self, request: &Request) -> Vec<u8> {
        serde_json::to_vec(request).expect("Failed to serialize request")
    }

    fn deserialize_request(&self, bytes: &[u8]) -> Result<Request, String> {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    }

    fn serialize_response(&self, response: &Response) -> Vec<u8> {
        let mut serialized = serde_json::to_vec(response).expect("Failed to serialize response");
        serialized.push(b'\n');
        serialized
    }

    fn deserialize_response(&self, bytes: &[u8]) -> Result<Response, String> {
        let response_str = String::from_utf8_lossy(bytes)
            .trim_end_matches('\n')
            .to_string();

        serde_json::from_str(&response_str).map_err(|e| e.to_string())
    }
}

pub struct SimpleSerializer;

impl Serializer for SimpleSerializer {
    fn serialize_request(&self, request: &Request) -> Vec<u8> {
        format!(
            "{} {} {}",
            request.command.clone().unwrap_or_default(),
            request.key.clone().unwrap_or_default(),
            request.value.clone().unwrap_or_default()
        )
        .into_bytes()
    }

    fn deserialize_request(&self, bytes: &[u8]) -> Result<Request, String> {
        let request_str = String::from_utf8_lossy(bytes).to_string();
        let parts: Vec<&str> = request_str.split_whitespace().collect();
        if parts.len() < 2 {
            Err("Invalid request".to_string())
        } else {
            Ok(Request {
                command: Some(parts[0].to_string()),
                key: Some(parts[1].to_string()),
                value: if parts.len() > 2 {
                    Some(parts[2].to_string())
                } else {
                    None
                },
            })
        }
    }

    fn serialize_response(&self, response: &Response) -> Vec<u8> {
        let mut serialized = response.message.clone().into_bytes();
        serialized.push(b'\n');
        serialized
    }

    fn deserialize_response(&self, bytes: &[u8]) -> Result<Response, String> {
        let response_str = String::from_utf8_lossy(bytes)
            .trim_end_matches('\n')
            .to_string();

        Ok(Response {
            status: "OK".to_string(),
            message: response_str,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Server;
    use rstest::rstest;
    use std::future::Future;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

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

    #[rstest]
    #[case(5004)]
    #[tokio::test]
    async fn test_with_different_serializers(#[case] port: u16) {
        let address = format!("localhost:{}", port);
        run_server(&address).await;

        let json_serializer = Box::new(JsonSerializer) as Box<dyn Serializer>;
        let simple_serializer = Box::new(SimpleSerializer) as Box<dyn Serializer>;

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
            let response = client_task(&mut stream, put_request, json_serializer.as_ref()).await;
            let expected = Response {
                status: "OK".to_string(),
                message: "Inserted key 'foo' with value 'bar'".to_string(),
            };
            assert_eq!(response.unwrap(), expected);

            let response = client_task(&mut stream, get_request, simple_serializer.as_ref()).await;
            let expected = Response {
                status: "OK".to_string(),
                message: "bar".to_string(),
            };
            assert_eq!(response.unwrap(), expected);

            let response = client_task(&mut stream, delete_request, json_serializer.as_ref()).await;
            let expected = Response {
                status: "OK".to_string(),
                message: "Deleted key 'foo' with value 'bar'".to_string(),
            };
            assert_eq!(response.unwrap(), expected);
        })
        .await;
    }
}
