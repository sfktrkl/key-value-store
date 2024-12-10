use std::env;

#[tokio::main]
async fn main() {
    let port = env::args()
        .skip(1)
        .find(|arg| arg == "--port")
        .and_then(|_| env::args().nth(2))
        .unwrap_or_else(|| "5000".to_string());

    println!("Listening on port: {}", port);
}
