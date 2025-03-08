//! Configuration module for the Vertex AI Setup Tool
//!
//! This module provides configuration structures and utilities for the tool.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Configuration for the Vertex AI setup tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The Google Cloud project ID
    pub project_id: String,
    /// The region for Vertex AI services
    pub region: String,
    /// The model to use for testing
    pub model: String,
    /// Whether to enable verbose output
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            region: "us-central1".to_string(),
            model: "gemini-pro".to_string(),
            verbose: false,
        }
    }
}

/// Environment variables used by the tool
pub mod env {
    /// The Google Cloud project ID environment variable
    pub const PROJECT_ID: &str = "VERTEX_AI_PROJECT_ID";
    /// The Google Cloud credentials environment variable
    pub const CREDENTIALS: &str = "GOOGLE_APPLICATION_CREDENTIALS";
}

/// Error types for configuration
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Error when environment variable is missing
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
    /// Error when configuration is invalid
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Result type for configuration operations
pub type ConfigResult<T> = Result<T>;
