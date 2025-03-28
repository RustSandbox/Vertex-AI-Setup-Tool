use anyhow::{Context, Result};
use serde_json::Value;
use std::process::Command;

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
    let services: Value =
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
