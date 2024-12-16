use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub command: String,
    pub key: Option<String>,
    pub value: Option<String>,
}

impl Request {
    pub fn deserialize(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("Failed to deserialize Request: {}", e))
    }
}

#[cfg(test)]
impl Request {
    pub fn to_string(&self) -> String {
        serde_json::to_string(self).expect("Failed to serialize request")
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    pub status: String,
    pub message: String,
}

impl Response {
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize Response")
    }
}
