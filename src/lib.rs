//! # Vertex AI Setup Tool Library
//!
//! A Rust library for setting up and testing Google Cloud Vertex AI integration.
//! This library provides the core functionality for the `hvertex` command-line tool.
//!
//! ## Features
//!
//! - Automatic service enablement
//! - Model discovery
//! - Authentication setup
//! - Environment management
//! - API testing
//!
//! ## Example
//!
//! ```rust,no_run
//! use hvertex::{Config, ensure_vertex_ai_service, list_vertex_ai_models};
//!
//! let config = Config::default();
//! ensure_vertex_ai_service(&config.project_id)?;
//! let models = list_vertex_ai_models(&config.project_id, &config.region)?;
//! Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Configuration
//!
//! The library can be configured through the `Config` struct:
//!
//! ```rust
//! use hvertex::Config;
//!
//! let config = Config {
//!     project_id: "my-project-id".to_string(),
//!     region: "us-central1".to_string(),
//!     model: "gemini-pro".to_string(),
//!     verbose: false,
//! };
//! ```
//!
//! ## Environment Variables
//!
//! The following environment variables are used:
//!
//! - `VERTEX_AI_PROJECT_ID`: Your Google Cloud project ID
//! - `GOOGLE_APPLICATION_CREDENTIALS`: Path to your service account key file
//!
//! ## Error Handling
//!
//! The library uses `anyhow::Result` for error handling, providing detailed error messages
//! and context for debugging.
//!
//! ## License
//!
//! This project is licensed under the MIT License.

pub mod auth;
pub mod models;
pub mod pdf;
pub mod setup;
pub mod vertex_ai;

// Re-export commonly used items
pub use auth::{get_access_token, setup_authentication};
pub use models::list_vertex_ai_models;
pub use pdf::extract_data_from_pdf_v2;
pub use setup::{ensure_vertex_ai_service, test_vertex_ai_api_call};
pub use vertex_ai::VertexAIRequest;

/// Re-export anyhow::Result for convenience
pub use anyhow::Result;
