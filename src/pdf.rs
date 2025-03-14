use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::env;

use crate::auth;
use crate::vertex_ai::VertexAIRequest;

/// Modified extract_data_from_pdf function to use the new VertexAIRequest struct
pub async fn extract_data_from_pdf_v2(
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
    let access_token = auth::get_access_token()?;

    // Set up the HTTP client
    let client = reqwest::Client::new();

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
        .await
        .context("Failed to make Vertex AI API request")?;

    // Check if the request was successful
    let status = response.status();
    if !status.is_success() {
        // If the request failed, return the error
        let error_text = response
            .text()
            .await
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
        .await
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
