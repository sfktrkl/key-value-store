use key_value_store::serialization::{
    JsonSerializer, Request, Response, Serializer, SimpleSerializer,
};
use key_value_store::server::Server;
use rstest::rstest;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const DURATION: core::time::Duration = Duration::from_millis(100);

async fn run_server(address: &str) {
    let mut server = Server::new(address, vec![], 1);
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
