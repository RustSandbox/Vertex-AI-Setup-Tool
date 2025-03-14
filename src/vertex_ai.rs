//! Vertex AI module for the setup tool
//!
//! This module provides functionality for interacting with Google Cloud Vertex AI services.

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::process::Command;

/// Structured representation of a Vertex AI API request
///
/// This set of structs represents the complete request body for the Vertex AI API,
/// making it easier to customize models, prompts, and other settings in a type-safe way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VertexAIRequest {
    /// The user's messages/content for the model
    pub contents: Vec<ContentItem>,
    /// System instructions to guide the model's behavior
    pub system_instruction: SystemInstruction,
    /// Configuration for the generation process
    pub generation_config: GenerationConfig,
    /// Safety settings to control content filtering
    pub safety_settings: Vec<SafetySetting>,
    /// Additional tools to enable for the model
    pub tools: Vec<Tool>,
}

/// Represents a content item in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    /// The role of the sender (e.g., "user", "assistant")
    pub role: String,
    /// The actual content parts (text, images, etc.)
    pub parts: Vec<ContentPart>,
}

/// Represents a part of a content item (text, inline data, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentPart {
    /// Plain text part
    Text { text: String },
    /// Inline data part (for PDFs, images, etc.)
    InlineData { inline_data: InlineData },
}

/// Represents inline data like PDFs, images, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineData {
    /// The MIME type of the data
    pub mime_type: String,
    /// The base64-encoded data
    pub data: String,
}

/// System instructions to guide the model's behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstruction {
    /// The parts of the system instruction
    pub parts: Vec<SystemInstructionPart>,
}

/// A part of the system instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInstructionPart {
    /// The text of the system instruction
    pub text: String,
}

/// Configuration for the generation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// The modalities for the response (e.g., ["TEXT"])
    pub response_modalities: Vec<String>,
    /// The temperature for generation (higher = more random)
    pub temperature: f32,
    /// The maximum number of tokens to generate
    pub max_output_tokens: u32,
    /// The top-p value for nucleus sampling
    pub top_p: f32,
}

/// Safety settings to control content filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    /// The category of harmful content to filter
    pub category: String,
    /// The threshold for filtering (e.g., "OFF", "LOW", "MEDIUM", "HIGH")
    pub threshold: String,
}

/// Additional tools to enable for the model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Tool {
    /// Google Search tool
    GoogleSearch { google_search: GoogleSearch },
}

/// Google Search tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearch {}

impl VertexAIRequest {
    /// Creates a new request for extracting data from a PDF
    ///
    /// # Arguments
    ///
    /// * `pdf_base64` - The base64-encoded PDF data
    /// * `prompt` - The text prompt for extraction instructions
    /// * `system_instruction` - Optional system instruction (uses default if None)
    ///
    /// # Returns
    ///
    /// * A new `VertexAIRequest` configured for PDF data extraction
    pub fn new_pdf_extraction(
        pdf_base64: &str,
        prompt: &str,
        system_instruction: Option<&str>,
    ) -> Self {
        let system_text = system_instruction.unwrap_or(
            "You are a data extractor specializing in insurance-related documents. You are an expert at extracting all data which can be extracted from any PDF, including data accessible through Optical Character Recognition (OCR)."
        );

        VertexAIRequest {
            contents: vec![ContentItem {
                role: "user".to_string(),
                parts: vec![
                    ContentPart::InlineData {
                        inline_data: InlineData {
                            mime_type: "application/pdf".to_string(),
                            data: pdf_base64.to_string(),
                        },
                    },
                    ContentPart::Text {
                        text: prompt.to_string(),
                    },
                ],
            }],
            system_instruction: SystemInstruction {
                parts: vec![SystemInstructionPart {
                    text: system_text.to_string(),
                }],
            },
            generation_config: GenerationConfig {
                response_modalities: vec!["TEXT".to_string()],
                temperature: 2.0,
                max_output_tokens: 8192,
                top_p: 0.95,
            },
            safety_settings: vec![
                SafetySetting {
                    category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                    threshold: "OFF".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                    threshold: "OFF".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                    threshold: "OFF".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_HARASSMENT".to_string(),
                    threshold: "OFF".to_string(),
                },
            ],
            tools: vec![Tool::GoogleSearch {
                google_search: GoogleSearch {},
            }],
        }
    }

    /// Sets a custom temperature for generation
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.generation_config.temperature = temperature;
        self
    }

    /// Sets a custom max output tokens limit
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.generation_config.max_output_tokens = max_tokens;
        self
    }

    /// Sets a custom top-p value
    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.generation_config.top_p = top_p;
        self
    }
}

/// Represents a Vertex AI model
#[derive(Debug, Deserialize, Clone)]
pub struct VertexAIModel {
    /// The name of the model
    pub name: String,
    /// The display name of the model
    pub display_name: String,
    /// The description of the model
    #[serde(default)]
    pub description: String,
}

/// Modified extract_data_from_pdf function to use the new VertexAIRequest struct
pub fn extract_data_from_pdf_v2(
    pdf_base64: &str,
    prompt: Option<&str>,
    system_instruction: Option<&str>,
    project_id: Option<String>,
    location_id: Option<&str>,
    model_id: Option<&str>,
) -> Result<serde_json::Value> {
    // Get the project ID, location ID, and model ID with default values
    let project_id = match project_id {
        Some(id) => id,
        None => env::var("VERTEX_AI_PROJECT_ID")
            .context("Project ID not provided and VERTEX_AI_PROJECT_ID not set")?,
    };
    let location_id = location_id.unwrap_or("us-central1");
    let model_id = model_id.unwrap_or("gemini-2.0-flash-exp");
    let api_endpoint = format!("{}-aiplatform.googleapis.com", location_id);

    println!("Extracting data from PDF using Vertex AI {}...", model_id);

    // Get access token for API authentication
    let access_token = crate::auth::get_access_token()?;

    // Set up the HTTP client
    let client = reqwest::blocking::Client::new();

    // Construct the API URL
    let api_url = format!(
        "https://{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
        api_endpoint, project_id, location_id, model_id
    );

    // Set up request headers
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", access_token))
            .context("Failed to create authorization header")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    // Use the default prompt or a custom one
    let default_prompt = "read this file give all data in josn format. you need to be smart key mininingfull choise and a fild for acuracy score . in and contacrt there diifrent information related to girent people like adress of company or adresss of indidual who signe contarct. those need to be seperated.";
    let prompt_text = prompt.unwrap_or(default_prompt);

    // Create the request using our new struct
    let request = VertexAIRequest::new_pdf_extraction(pdf_base64, prompt_text, system_instruction);

    // Make the API request
    let response = client
        .post(api_url)
        .headers(headers)
        .json(&request)
        .send()
        .context("Failed to make Vertex AI API request")?;

    // Check if the request was successful
    let status = response.status();
    if !status.is_success() {
        // If the request failed, return the error
        let error_text = response
            .text()
            .unwrap_or_else(|_| "Unable to get error details".to_string());
        return Err(anyhow::anyhow!(
            "API request failed with status code {}: {}",
            status,
            error_text
        ));
    }

    // Parse the response
    let response_json: Value = response
        .json()
        .context("Failed to parse API response as JSON")?;

    // Extract the generated text and parse it as JSON
    if let Some(candidates) = response_json["candidates"].as_array() {
        if let Some(content) = candidates
            .first()
            .and_then(|c| c["content"]["parts"].as_array())
            .and_then(|parts| parts.first())
            .and_then(|p| p["text"].as_str())
        {
            // Try to parse the response text as JSON
            match serde_json::from_str::<Value>(content) {
                Ok(json_data) => {
                    return Ok(json_data);
                }
                Err(e) => {
                    // If parsing as JSON fails, return the raw text as a JSON string
                    println!(
                        "Warning: Could not parse response as JSON ({}). Returning raw text.",
                        e
                    );
                    return Ok(json!({ "raw_text": content }));
                }
            }
        }
    }

    // If we couldn't extract the response, return an error
    Err(anyhow::anyhow!(
        "Failed to extract data from the API response"
    ))
}

/// Ensures the Vertex AI service is enabled in the project
///
/// # Arguments
///
/// * `project_id` - The Google Cloud project ID
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Success or error status
///
/// # Example
///
/// ```rust,no_run
/// use hvertex::ensure_vertex_ai_service;
///
/// let project_id = "my-project-id";
/// ensure_vertex_ai_service(project_id)?;
/// Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn ensure_vertex_ai_service(project_id: &str) -> Result<()> {
    // Check if Vertex AI service is enabled
    let output = Command::new("gcloud")
        .args(["services", "list", "--project", project_id, "--format=json"])
        .output()
        .context("Failed to execute gcloud services list command")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to list services: {}", error));
    }

    // Parse the JSON output to check if Vertex AI is enabled
    let services: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse services list output")?;

    let vertex_ai_enabled = services
        .as_array()
        .map(|arr| {
            arr.iter().any(|service| {
                service["config"]["name"]
                    .as_str()
                    .is_some_and(|name| name.contains("aiplatform.googleapis.com"))
            })
        })
        .unwrap_or(false);

    if !vertex_ai_enabled {
        // Enable Vertex AI service
        let enable_output = Command::new("gcloud")
            .args([
                "services",
                "enable",
                "aiplatform.googleapis.com",
                "--project",
                project_id,
            ])
            .output()
            .context("Failed to enable Vertex AI service")?;

        if !enable_output.status.success() {
            let error = String::from_utf8_lossy(&enable_output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to enable Vertex AI service: {}",
                error
            ));
        }
    }

    Ok(())
}

/// Lists available Vertex AI models in the project
///
/// # Arguments
///
/// * `project_id` - The Google Cloud project ID
/// * `region` - The region to list models from
///
/// # Returns
///
/// * `Result<Vec<String>, anyhow::Error>` - List of model names or error
///
/// # Example
///
/// ```rust,no_run
/// use hvertex::list_vertex_ai_models;
///
/// let project_id = "my-project-id";
/// let region = "us-central1";
/// let models = list_vertex_ai_models(project_id, region)?;
/// Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn list_vertex_ai_models(project_id: &str, region: &str) -> Result<Vec<String>> {
    // List Vertex AI models
    let output = Command::new("gcloud")
        .args([
            "ai",
            "models",
            "list",
            "--region",
            region,
            "--project",
            project_id,
            "--format=json",
        ])
        .output()
        .context("Failed to execute gcloud ai models list command")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("not find any resources") {
            return Ok(Vec::new());
        }
        return Err(anyhow::anyhow!("Failed to list models: {}", error));
    }

    // Parse the JSON output
    let models: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).context("Failed to parse models list output")?;

    // Extract model names
    let model_names = models
        .iter()
        .filter_map(|model| model["name"].as_str())
        .map(String::from)
        .collect();

    Ok(model_names)
}

/// Sets up authentication for Vertex AI
///
/// # Arguments
///
/// * `project_id` - The Google Cloud project ID
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Success or error status
///
/// # Example
///
/// ```rust,no_run
/// use hvertex::setup_authentication;
///
/// let project_id = "my-project-id";
/// setup_authentication(project_id)?;
/// Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn setup_authentication(project_id: &str) -> Result<()> {
    // Set up application default credentials
    let output = Command::new("gcloud")
        .args([
            "auth",
            "application-default",
            "login",
            "--project",
            project_id,
        ])
        .output()
        .context("Failed to set up authentication")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to set up authentication: {}",
            error
        ));
    }

    Ok(())
}

/// Tests the Vertex AI API with a sample request
///
/// # Arguments
///
/// * `project_id` - The Google Cloud project ID
/// * `model` - The model to test with
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Success or error status
///
/// # Example
///
/// ```rust,no_run
/// use hvertex::test_vertex_ai_api_call;
///
/// let project_id = "my-project-id";
/// let model = "gemini-pro";
/// test_vertex_ai_api_call(project_id, model)?;
/// Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn test_vertex_ai_api_call(project_id: &str, model: &str) -> Result<()> {
    // Get access token
    let token_output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .context("Failed to get access token")?;

    if !token_output.status.success() {
        let error = String::from_utf8_lossy(&token_output.stderr);
        return Err(anyhow::anyhow!("Failed to get access token: {}", error));
    }

    let access_token = String::from_utf8(token_output.stdout)
        .context("Failed to parse access token")?
        .trim()
        .to_string();

    // Construct the API URL
    let api_url = format!(
        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
        "us-central1", project_id, "us-central1", model
    );

    // Create a test request
    let request_body = serde_json::json!({
        "contents": [
            {
                "role": "user",
                "parts": [
                    {
                        "text": "Hello, this is a test message."
                    }
                ]
            }
        ]
    });

    // Make the API request using reqwest
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&api_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .context("Failed to make API request")?;

    if !response.status().is_success() {
        let error_text = response
            .text()
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(anyhow::anyhow!("API request failed: {}", error_text));
    }

    Ok(())
}
