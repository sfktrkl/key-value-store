use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Request {
    pub command: String,
    pub key: Option<String>,
    pub value: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Response {
    pub status: String,
    pub message: String,
}

pub trait Serializer {
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
        serde_json::to_vec(response).expect("Failed to serialize response")
    }

    #[cfg(test)]
    fn deserialize_response(&self, bytes: &[u8]) -> Result<Response, String> {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    }
}
