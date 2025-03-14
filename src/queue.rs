use anyhow::Result;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{Mutex, Semaphore},
    time::sleep,
};

/// Configuration for the request queue
#[derive(Debug)]
pub struct QueueConfig {
    /// Maximum number of tokens in the bucket
    pub max_tokens: usize,
    /// Number of tokens added per refill
    pub refill_tokens: usize,
    /// Time interval between token refills
    pub refill_interval: Duration,
    /// Maximum number of concurrent requests
    pub max_concurrent_requests: usize,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_tokens: 1000000,                      // 1 million tokens to handle large PDFs
            refill_tokens: 100000,                    // Refill 100k tokens per interval
            refill_interval: Duration::from_secs(60), // Refill every minute
            max_concurrent_requests: 3,
        }
    }
}

/// Token bucket implementation for rate limiting
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens
    tokens: usize,
    /// Maximum number of tokens
    max_tokens: usize,
    /// Number of tokens added per refill
    refill_tokens: usize,
    /// Time of last refill
    last_refill: Instant,
    /// Time interval between refills
    refill_interval: Duration,
}

impl TokenBucket {
    fn new(config: &QueueConfig) -> Self {
        Self {
            tokens: config.max_tokens,
            max_tokens: config.max_tokens,
            refill_tokens: config.refill_tokens,
            last_refill: Instant::now(),
            refill_interval: config.refill_interval,
        }
    }

    /// Refills the token bucket based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let intervals = (elapsed.as_nanos() / self.refill_interval.as_nanos()) as usize;

        if intervals > 0 {
            self.tokens = (self.tokens + self.refill_tokens * intervals).min(self.max_tokens);
            self.last_refill = now;
        }
    }

    /// Attempts to consume a token
    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }
}

/// Request queue with rate limiting
pub struct RequestQueue {
    token_bucket: Arc<Mutex<TokenBucket>>,
    semaphore: Arc<Semaphore>,
}

impl RequestQueue {
    /// Creates a new request queue with the specified configuration
    pub fn new(config: QueueConfig) -> Self {
        Self {
            token_bucket: Arc::new(Mutex::new(TokenBucket::new(&config))),
            semaphore: Arc::new(Semaphore::new(config.max_concurrent_requests)),
        }
    }

    /// Executes a request with rate limiting and concurrency control
    pub async fn execute<F, T>(&self, request: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + Clone + 'static,
        T: Send + 'static,
    {
        // Acquire a permit from the semaphore
        let _permit = self.semaphore.acquire().await?;

        loop {
            // Try to acquire a token
            let can_proceed = {
                let mut bucket = self.token_bucket.lock().await;
                bucket.try_consume()
            };

            if can_proceed {
                // Clone the request for this attempt
                let request = request.clone();

                // Execute the request
                match request() {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        // If it's a rate limit error (429), wait and retry
                        if e.to_string().contains("429") {
                            sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                        return Err(e);
                    }
                }
            }

            // If no token is available, wait before retrying
            sleep(Duration::from_millis(100)).await;
        }
    }

    /// Returns the current number of available tokens
    pub async fn available_tokens(&self) -> usize {
        let bucket = self.token_bucket.lock().await;
        bucket.tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_token_bucket_refill() {
        let config = QueueConfig {
            max_tokens: 10,
            refill_tokens: 2,
            refill_interval: Duration::from_millis(100),
            max_concurrent_requests: 3,
        };

        let mut bucket = TokenBucket::new(&config);
        assert_eq!(bucket.tokens, 10);

        // Consume all tokens
        for _ in 0..10 {
            assert!(bucket.try_consume());
        }
        assert_eq!(bucket.tokens, 0);

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(200)).await;
        bucket.refill();
        assert_eq!(bucket.tokens, 4);
    }
}
