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
    #[cfg(test)]
    fn serialize_request(&self, request: &Request) -> Vec<u8>;

    fn deserialize_request(&self, bytes: &[u8]) -> Result<Request, String>;

    fn serialize_response(&self, response: &Response) -> Vec<u8>;

    #[cfg(test)]
    fn deserialize_response(&self, bytes: &[u8]) -> Result<Response, String>;
}

pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    #[cfg(test)]
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

    #[cfg(test)]
    fn deserialize_response(&self, bytes: &[u8]) -> Result<Response, String> {
        let response_str = String::from_utf8_lossy(bytes)
            .trim_end_matches('\n')
            .to_string();

        serde_json::from_str(&response_str).map_err(|e| e.to_string())
    }
}

pub struct SimpleSerializer;

impl Serializer for SimpleSerializer {
    #[cfg(test)]
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

    #[cfg(test)]
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
