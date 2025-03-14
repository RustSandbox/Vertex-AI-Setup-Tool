mod auth;
mod models;
mod pdf;
mod setup;
mod vertex_ai;

/// # Vertex AI Setup Tool
///
/// A powerful command-line tool for setting up and testing Google Cloud Vertex AI integration.
/// This tool automates the process of enabling Vertex AI services, managing authentication,
/// and testing API calls.
///
/// ## Features
///
/// - Automatic service enablement
/// - Model discovery
/// - Authentication setup
/// - Environment management
/// - Rich terminal interface
/// - API testing
///
/// ## Example
///
/// ```bash
/// cargo install hvertex
/// hvertex
/// ```
///
/// ## Configuration
///
/// The tool can be configured through environment variables:
///
/// - `VERTEX_AI_PROJECT_ID`: Your Google Cloud project ID
/// - `GOOGLE_APPLICATION_CREDENTIALS`: Path to your service account key file
///
/// ## License
///
/// This project is licensed under the MIT License.
use anyhow::{Context, Result};
use base64::engine::general_purpose;
use base64::Engine;
use colored::Colorize;
use regex::Regex;
use serde_json::Value;
use std::{env, fs::File, io::Write};

use crate::auth::get_access_token;
use crate::models::list_vertex_ai_models;
use crate::pdf::extract_data_from_pdf_v2;
use crate::setup::{ensure_vertex_ai_service, test_vertex_ai_api_call};

/// Main entry point for the application
///
/// # Returns
///
/// * `Result<(), Box<dyn std::error::Error>>` - Success or error status
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Print a welcome message with styling
    println!("{}", "Vertex AI Setup Tool".green().bold());
    println!("{}", "===================".green());
    println!("This tool will help you set up Google Cloud Vertex AI for your project.");
    println!();

    // Load existing environment variables if any
    load_environment()?;

    // Step 1: Ensure Vertex AI is enabled for the project
    //ensure_vertex_ai_project()?;

    // Step 2: List Vertex AI models
    //let _models = list_vertex_ai_models()?;

    // Step 3: Setup authentication for API access
    //setup_authentication()?;

    // Step 4: Check environment variables
    check_environment_variables()?;

    // Step 5: Test the Vertex AI API with a simple text generation request
    //test_vertex_ai_api_call()?;

    // Step 6: Print instructions for using the authentication in the future
    //print_authentication_instructions();
    let pdf_path = "/Users/hamzeghalebi/project/remolab/giva/pdfproject/data/103/2_707_VIE0062967625W4LC011GCOMM____2018-09-06-13.44.44.3900001.pdf";
    let pdf_bytes = std::fs::read(pdf_path).expect("Failed to read PDF file");
    let pdf_base64 = general_purpose::STANDARD.encode(pdf_bytes);

    // Use the new structured API
    println!("\n{}", "Using structured API request:".blue().bold());

    // Create a custom prompt
    let custom_prompt = "Extract all data from this insurance document into JSON format. Include fields for document type, contract details, personal information, and payment schedule. Add an accuracy_score field (0.0-1.0) for each extracted data point.";

    // Extract data from the PDF with the new function and custom prompt
    let api_response = extract_data_from_pdf_v2(
        &pdf_base64,
        Some(custom_prompt),
        None, // Use default system instruction
        None, // Use default project ID from env
        None, // Use default location
        None, // Use default model
    )?;

    // Process the response
    println!("Raw API response received. Processing...");

    // Check if we need to extract JSON from raw text
    if let Some(raw_text) = api_response.get("raw_text").and_then(|v| v.as_str()) {
        println!("Detected raw text response. Extracting JSON data...");

        // Extract the JSON from the raw text
        match extract_json_from_raw_text(raw_text) {
            Ok(extracted_json) => {
                println!("Successfully extracted JSON data!");
                println!("Extracted data sample:");

                // Print a sample of the extracted data
                if let Some(doc_type) = extracted_json.get("document_type").and_then(|v| v.as_str())
                {
                    println!("Document Type: {}", doc_type.cyan());
                }

                if let Some(contract_num) = extracted_json
                    .get("contract_number")
                    .and_then(|v| v.as_str())
                {
                    println!("Contract Number: {}", contract_num.cyan());
                }

                // Write the pretty-printed JSON to a file for inspection
                let json_str = serde_json::to_string_pretty(&extracted_json)?;
                let output_path = "extracted_data_structured.json";
                std::fs::write(output_path, json_str)?;
                println!("Full extracted data written to: {}", output_path.cyan());
            }
            Err(e) => {
                println!("Error extracting JSON: {}", e);
                println!("Raw text: {}", raw_text);
            }
        }
    } else {
        // The response is already in JSON format
        println!("API response is already in JSON format:");
        println!("{:#?}", api_response);
    }

    Ok(())
}

/// Extracts and parses JSON data from raw text that contains Markdown code blocks
///
/// This function is designed to handle responses from the Vertex AI API that
/// may return JSON data wrapped in Markdown code blocks (```json ... ```).
/// It will extract the JSON content and parse it into a proper serde_json::Value.
///
/// # Arguments
///
/// * `raw_text` - A string slice containing the raw text with potential JSON data in code blocks
///
/// # Returns
///
/// * `Result<serde_json::Value, anyhow::Error>` - The parsed JSON value or an error
///
/// # Example
///
/// ```rust
/// let response = extract_data_from_pdf(&pdf_base64, None, None, None)?;
///
/// // If the response contains raw_text with JSON in backticks
/// if let Some(raw_text) = response.get("raw_text") {
///     if let Some(text) = raw_text.as_str() {
///         let parsed_json = extract_json_from_raw_text(text)?;
///         println!("{}", serde_json::to_string_pretty(&parsed_json)?);
///     }
/// }
/// ```
pub fn extract_json_from_raw_text(raw_text: &str) -> Result<Value> {
    // First, check if the input is a JSON object with a "raw_text" field
    if let Ok(parsed) = serde_json::from_str::<Value>(raw_text) {
        if let Some(inner_text) = parsed.get("raw_text").and_then(|v| v.as_str()) {
            // If we have a "raw_text" field, use its value as our raw text
            return extract_json_from_raw_text(inner_text);
        }
    }

    // Create a regex to match JSON content within triple backticks
    // The (?s) modifier enables "dot matches newline" mode
    let re = Regex::new(r"```(?:json)?\s*([\s\S]*?)\s*```").context("Failed to compile regex")?;

    // Try to find a match
    if let Some(captures) = re.captures(raw_text) {
        if let Some(json_str) = captures.get(1) {
            // Parse the extracted JSON string
            return serde_json::from_str::<Value>(json_str.as_str())
                .context("Failed to parse extracted JSON");
        }
    }

    // If no code blocks were found, try to parse the entire text as JSON
    serde_json::from_str::<Value>(raw_text)
        .context("Failed to parse text as JSON and no code blocks were found")
}

/// Loads environment variables from a .env file if it exists
fn load_environment() -> Result<()> {
    match dotenv::dotenv() {
        Ok(_) => println!("Loaded environment from .env file"),
        Err(e) => println!("No .env file found: {}", e),
    }
    Ok(())
}

/// Checks if required environment variables are set
fn check_environment_variables() -> Result<()> {
    println!("\n{}", "Checking environment variables...".blue().bold());

    // Check for VERTEX_AI_PROJECT_ID
    match env::var("VERTEX_AI_PROJECT_ID") {
        Ok(project_id) => println!("✅ VERTEX_AI_PROJECT_ID is set: {}", project_id.cyan()),
        Err(_) => println!("❌ VERTEX_AI_PROJECT_ID is not set"),
    }

    // Check for Google Cloud credentials
    match env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        Ok(creds) => println!("✅ GOOGLE_APPLICATION_CREDENTIALS is set: {}", creds.cyan()),
        Err(_) => println!("❌ GOOGLE_APPLICATION_CREDENTIALS is not set"),
    }

    // Check for access token
    match get_access_token() {
        Ok(_) => println!("✅ Access token is available"),
        Err(_) => println!("❌ Access token is not available"),
    }

    Ok(())
}
