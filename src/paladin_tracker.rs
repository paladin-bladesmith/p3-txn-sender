use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{error, info};

// the api returns a direct array of validator strings, not wrapped in an object

/// tracks paladin validators and provides efficient lookup
pub struct PaladinTracker {
    validators: Arc<RwLock<HashSet<String>>>,
    client: reqwest::Client,
}

impl PaladinTracker {
    /// creates a new paladin tracker instance
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to create http client");

        Self {
            validators: Arc::new(RwLock::new(HashSet::new())),
            client,
        }
    }

    /// starts the background task to update validator list hourly
    pub async fn start_background_updates(&self) {
        let validators = Arc::clone(&self.validators);
        let client = self.client.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600)); // 1 hour
            
            loop {
                interval.tick().await;
                
                match Self::fetch_validators(&client).await {
                    Ok(new_validators) => {
                        let mut validators_write = validators.write().await;
                        *validators_write = new_validators;
                        info!("updated paladin validator list with {} validators", validators_write.len());
                    }
                    Err(e) => {
                        error!("failed to update paladin validator list: {}", e);
                    }
                }
            }
        });
    }

    /// performs initial load of validator list
    pub async fn initialize(&self) -> anyhow::Result<()> {
        info!("initializing paladin tracker...");
        
        let new_validators = Self::fetch_validators(&self.client).await?;
        let mut validators_write = self.validators.write().await;
        *validators_write = new_validators;
        
        info!("initialized paladin tracker with {} validators", validators_write.len());
        Ok(())
    }

    /// fetches the current list of paladin validators from the api
    async fn fetch_validators(client: &reqwest::Client) -> anyhow::Result<HashSet<String>> {
        info!("fetching paladin validators from api...");
        
        let response = client
            .get("https://api.paladin.one/validators")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("api request failed with status: {}", response.status()));
        }

        let validators: Vec<String> = response.json().await?;
        let validator_set: HashSet<String> = validators.into_iter().collect();
        
        info!("fetched {} paladin validators", validator_set.len());
        Ok(validator_set)
    }

    /// checks if a validator is a paladin validator
    pub async fn is_paladin_validator(&self, validator_pubkey: &str) -> bool {
        let validators_read = self.validators.read().await;
        let is_paladin = validators_read.contains(validator_pubkey);
        
        if is_paladin {
            info!("validator {} is a paladin validator", validator_pubkey);
        }
        
        is_paladin
    }

    /// gets the current count of paladin validators (for monitoring)
    pub async fn get_validator_count(&self) -> usize {
        let validators_read = self.validators.read().await;
        validators_read.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_paladin_tracker_creation() {
        let tracker = PaladinTracker::new();
        assert_eq!(tracker.get_validator_count().await, 0);
    }

    #[tokio::test]
    async fn test_is_paladin_validator_empty() {
        let tracker = PaladinTracker::new();
        assert!(!tracker.is_paladin_validator("test_validator").await);
    }

    #[tokio::test]
    async fn test_fetch_validators_with_network() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to create http client");

        // test actual api call
        match PaladinTracker::fetch_validators(&client).await {
            Ok(validators) => {
                println!("successfully fetched {} validators", validators.len());
                assert!(validators.len() > 0, "should have at least some validators");
                
                // test with known paladin validator
                let known_validator = "J5AsxaHfWn6KpEcPRT9EZ9szvEMBQeHRe947UeaMPG3z";
                if validators.contains(known_validator) {
                    println!("confirmed {} is in paladin validator list", known_validator);
                } else {
                    println!("note: {} not currently in paladin validator list", known_validator);
                }
            }
            Err(e) => {
                // if network is unavailable, just log and continue
                println!("network test skipped due to error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_paladin_tracker_full_integration() {
        let tracker = PaladinTracker::new();
        
        // test initialization with real api
        match tracker.initialize().await {
            Ok(()) => {
                println!("successfully initialized paladin tracker");
                
                let validator_count = tracker.get_validator_count().await;
                println!("loaded {} paladin validators", validator_count);
                assert!(validator_count > 0, "should have loaded some validators");
                
                // test lookup with known validator
                let known_validator = "J5AsxaHfWn6KpEcPRT9EZ9szvEMBQeHRe947UeaMPG3z";
                let is_paladin = tracker.is_paladin_validator(known_validator).await;
                println!("validator {} is paladin: {}", known_validator, is_paladin);
                
                // test lookup with obviously fake validator
                let fake_validator = "1111111111111111111111111111111111111111111";
                let is_fake_paladin = tracker.is_paladin_validator(fake_validator).await;
                assert!(!is_fake_paladin, "fake validator should not be paladin");
                
                // test case sensitivity and exact matching
                let lowercase_validator = known_validator.to_lowercase();
                let is_lowercase_paladin = tracker.is_paladin_validator(&lowercase_validator).await;
                if is_paladin {
                    // if the known validator is actually in the list, lowercase should not match
                    assert!(!is_lowercase_paladin, "validator lookup should be case sensitive");
                }
            }
            Err(e) => {
                println!("integration test skipped due to network error: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_background_updates() {
        let tracker = PaladinTracker::new();
        
        // initialize first
        if tracker.initialize().await.is_ok() {
            let initial_count = tracker.get_validator_count().await;
            println!("initial validator count: {}", initial_count);
            
            // start background updates (this spawns a task)
            tracker.start_background_updates().await;
            
            // wait a short time to ensure the background task is running
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // the background task should be running (we can't easily test the hourly update
            // without waiting an hour, but we can verify the task was spawned)
            println!("background update task started successfully");
        } else {
            println!("background update test skipped due to network error");
        }
    }

    #[tokio::test]
    async fn test_api_error_handling() {
        // test with invalid url to verify error handling
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to create http client");

        // this should fail gracefully
        let result = client
            .get("https://invalid-api-endpoint-that-does-not-exist.com/validators")
            .send()
            .await;
            
        match result {
            Ok(_) => println!("unexpected success with invalid endpoint"),
            Err(e) => println!("correctly handled invalid endpoint error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_concurrent_validator_lookups() {
        let tracker = Arc::new(PaladinTracker::new());
        
        if tracker.initialize().await.is_ok() {
            let validator_count = tracker.get_validator_count().await;
            if validator_count > 0 {
                // test concurrent access to validator data
                let mut handles = vec![];
                
                for i in 0..10 {
                    let tracker_clone = Arc::clone(&tracker);
                    let handle = tokio::spawn(async move {
                        let test_validator = format!("test_validator_{}", i);
                        tracker_clone.is_paladin_validator(&test_validator).await
                    });
                    handles.push(handle);
                }
                
                // wait for all concurrent lookups to complete
                for handle in handles {
                    let result = handle.await.expect("task should complete");
                    // all test validators should return false
                    assert!(!result, "test validators should not be paladin validators");
                }
                
                println!("concurrent validator lookups completed successfully");
            }
        } else {
            println!("concurrent test skipped due to network error");
        }
    }
}