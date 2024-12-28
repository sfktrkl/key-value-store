use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Storage {
    data: Arc<RwLock<HashMap<String, String>>>,
}

impl Storage {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn put(&self, key: String, value: String) {
        let mut data = self.data.write().await;
        data.insert(key, value);
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let data = self.data.read().await;
        data.get(key).cloned()
    }

    pub async fn delete(&self, key: &str) -> Option<String> {
        let mut data = self.data.write().await;
        data.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_and_get() {
        let storage = Storage::new();
        storage.put("key1".to_string(), "value1".to_string()).await;
        storage.put("key2".to_string(), "value2".to_string()).await;

        assert_eq!(storage.get("key1").await, Some("value1".to_string()));
        assert_eq!(storage.get("key2").await, Some("value2".to_string()));
        assert_eq!(storage.get("key3").await, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let storage = Storage::new();
        storage.put("key1".to_string(), "value1".to_string()).await;
        storage.put("key2".to_string(), "value2".to_string()).await;

        assert_eq!(storage.delete("key1").await, Some("value1".to_string()));
        assert_eq!(storage.get("key1").await, None);
        assert_eq!(storage.get("key2").await, Some("value2".to_string()));
        assert_eq!(storage.delete("key3").await, None);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let storage = Storage::new();
        let key = "key1".to_string();

        let handles = (0..10).map(|i| {
            let storage = storage.clone();
            let key = key.clone();
            tokio::spawn(async move {
                let value = format!("value{}", i);
                storage.put(key.clone(), value.clone()).await;
                storage.get(&key).await
            })
        });

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }
    }
}
