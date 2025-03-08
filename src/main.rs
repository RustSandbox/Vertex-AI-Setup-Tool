use anyhow::{Context, Result};
use colored::Colorize;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    env,
    io::{self, Write},
    process::{Command, Output},
    fs::File,
    path::PathBuf,
};

/// Represents a Vertex AI model
#[derive(Debug, Deserialize, Clone)]
struct VertexAIModel {
    /// The name of the model
    name: String,
    /// The display name of the model
    display_name: String,
    /// The description of the model
    #[serde(default)]
    description: String,
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

/// Main entry point for the application
fn main() -> Result<()> {
    // Print a welcome message with styling
    println!("{}", "Vertex AI Setup Tool".green().bold());
    println!("{}", "===================".green());
    println!("This tool will help you set up Google Cloud Vertex AI for your project.");
    println!();

    // Load existing environment variables if any
    load_environment()?;

    // Step 1: Ensure Vertex AI is enabled for the project
    ensure_vertex_ai_project()?;

    // Step 2: List Vertex AI models
    let _models = list_vertex_ai_models()?;
    
    // Step 3: Setup authentication for API access
    setup_authentication()?;
    
    // Step 4: Check environment variables
    check_environment_variables()?;
    
    // Step 5: Test the Vertex AI API with a simple text generation request
    test_vertex_ai_api_call()?;
    
    // Step 6: Print instructions for using the authentication in the future
    print_authentication_instructions();

    Ok(())
}

/// Ensures that the Vertex AI service is enabled for the Google Cloud project
///
/// This function checks if the Vertex AI service (aiplatform.googleapis.com) is enabled.
/// If not, it attempts to enable it using the gcloud services enable command.
fn ensure_vertex_ai_project() -> Result<()> {
    println!("{}", "Step 1: Checking if Vertex AI is enabled...".blue().bold());

    // Execute gcloud services list command to check if Vertex AI is enabled
    let output = Command::new("gcloud")
        .args(["services", "list", "--format=json"])
        .output()
        .context("Failed to execute gcloud services list command")?;

    // Check if the command was successful
    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "gcloud services list command failed: {}",
            error_message
        ));
    }

    // Parse the JSON output
    let services: Vec<Value> = serde_json::from_slice(&output.stdout)
        .context("Failed to parse gcloud services list output")?;

    // Check if Vertex AI service is enabled
    let vertex_ai_enabled = services.iter().any(|service| {
        service["config"]["name"]
            .as_str()
            .unwrap_or("")
            .contains("aiplatform.googleapis.com")
    });

    if vertex_ai_enabled {
        println!("✅ Vertex AI service is already enabled");
    } else {
        println!("Vertex AI service is not enabled. Enabling it now...");
        
        // Execute gcloud services enable command to enable Vertex AI
        let enable_output = Command::new("gcloud")
            .args(["services", "enable", "aiplatform.googleapis.com"])
            .output()
            .context("Failed to execute gcloud services enable command")?;

        if !enable_output.status.success() {
            let error_message = String::from_utf8_lossy(&enable_output.stderr);
            return Err(anyhow::anyhow!(
                "Failed to enable Vertex AI service: {}",
                error_message
            ));
        }

        println!("✅ Vertex AI service has been enabled");
    }

    Ok(())
}

/// Lists available Vertex AI models in the project
///
/// This function retrieves a list of Vertex AI models available in the
/// specified region (default: us-central1) and returns them as a vector
/// of VertexAIModel structs.
fn list_vertex_ai_models() -> Result<Vec<VertexAIModel>> {
    println!("\n{}", "Step 2: Listing available Vertex AI models...".blue().bold());

    // Execute gcloud ai models list command
    let output = Command::new("gcloud")
        .args([
            "ai", 
            "models", 
            "list", 
            "--region=us-central1", 
            "--format=json"
        ])
        .output()
        .context("Failed to execute gcloud ai models list command")?;

    if !output.status.success() {
        // If the command fails but it's because no models are found, return an empty vector
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("not find any resources") {
            println!("ℹ️ No models found in the project");
            return Ok(Vec::new());
        }

        // Otherwise, return an error
        return Err(anyhow::anyhow!(
            "gcloud ai models list command failed: {}",
            error
        ));
    }

    // Parse the JSON output
    let models: Vec<VertexAIModel> = serde_json::from_slice(&output.stdout)
        .context("Failed to parse model list output")?;

    // Display the models
    if !models.is_empty() {
        println!("Found {} Vertex AI models:", models.len());
        for (idx, model) in models.iter().enumerate() {
            println!(
                "{}. {} ({})",
                idx + 1,
                model.display_name.cyan(),
                model.name
            );
            if !model.description.is_empty() {
                println!("   {}", model.description);
            }
        }
    } else {
        println!("ℹ️ No custom models found in the project");
        println!("You can still use Google's pre-built models like text-bison");
    }

    Ok(models)
}

/// Sets up authentication for the Vertex AI API
///
/// This function sets up authentication using gcloud auth application-default
/// login command and automatically sets up environment variables.
fn setup_authentication() -> Result<()> {
    println!("\n{}", "Step 3: Setting up authentication...".blue().bold());
    println!("For Vertex AI API access, we will use Application Default Credentials (ADC)");
    
    // Execute gcloud auth application-default login command with --quiet flag
    let output = Command::new("gcloud")
        .args(["auth", "application-default", "login", "--quiet"])
        .output()
        .context("Failed to execute gcloud auth application-default login command")?;

    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Authentication setup failed: {}",
            error_message
        ));
    }

    println!("✅ Authentication has been set up successfully");

    // Get project ID for future API calls
    let project_id = get_project_id()?;
    println!("✅ Using Google Cloud project: {}", project_id.cyan());
    
    // Set environment variables
    env::set_var("VERTEX_AI_PROJECT_ID", &project_id);
    
    // Create .env file in the project root
    let env_path = PathBuf::from(".env");
    let mut env_file = File::create(&env_path)
        .context("Failed to create .env file")?;
    
    // Write environment variables to .env file
    writeln!(env_file, "VERTEX_AI_PROJECT_ID={}", project_id)
        .context("Failed to write to .env file")?;
    
    // Get the path to the application default credentials
    let adc_path = Command::new("gcloud")
        .args(["auth", "application-default", "print-access-token"])
        .output()
        .context("Failed to get ADC path")?;
    
    if adc_path.status.success() {
        let adc_path_str = String::from_utf8_lossy(&adc_path.stdout).trim().to_string();
        writeln!(env_file, "GOOGLE_APPLICATION_CREDENTIALS={}", adc_path_str)
            .context("Failed to write ADC path to .env file")?;
    }

    println!("✅ Environment variables have been set and saved to .env file");
    println!("✅ You can now use these variables in your applications");

    Ok(())
}

/// Loads environment variables from .env file
///
/// This function loads environment variables from the .env file
/// if it exists, otherwise it uses the current environment.
fn load_environment() -> Result<()> {
    // Check if .env file exists
    if PathBuf::from(".env").exists() {
        // Load .env file
        dotenv::dotenv().context("Failed to load .env file")?;
        println!("✅ Loaded environment variables from .env file");
    }
    Ok(())
}

/// Gets the current Google Cloud project ID
///
/// This function retrieves the current Google Cloud project ID using
/// the gcloud config get-value project command.
fn get_project_id() -> Result<String> {
    let output = Command::new("gcloud")
        .args(["config", "get-value", "project"])
        .output()
        .context("Failed to execute gcloud config get-value project command")?;

    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to get project ID: {}",
            error_message
        ));
    }

    let project_id = String::from_utf8(output.stdout)
        .context("Failed to parse project ID")?
        .trim()
        .to_string();

    if project_id.is_empty() {
        return Err(anyhow::anyhow!("No Google Cloud project is set. Please run 'gcloud config set project YOUR_PROJECT_ID' to set a project."));
    }

    Ok(project_id)
}

/// Tests the Vertex AI API with a text generation request
///
/// This function makes a test API call to the Vertex AI API to
/// generate text using the Gemini Pro 2 model with Google Search grounding.
fn test_vertex_ai_api_call() -> Result<()> {
    println!("\n{}", "Step 4: Testing the Vertex AI API...".blue().bold());
    
    // Get access token for API authentication
    let access_token = get_access_token()?;
    
    // Get project ID from environment variable (set in setup_authentication)
    let project_id = env::var("VERTEX_AI_PROJECT_ID")
        .context("Project ID not found. Please make sure authentication is set up correctly.")?;
    
    println!("Making a test API call to the Vertex AI Gemini Pro 2 model with Google Search grounding...");
    
    // Set up the HTTP client
    let client = reqwest::blocking::Client::new();
    
    // Construct the API URL for Gemini Pro 2 model
    let api_url = format!(
        "https://us-central1-aiplatform.googleapis.com/v1/projects/{}/locations/us-central1/publishers/google/models/gemini-1.5-pro:generateContent",
        project_id
    );
    
    // Set up request headers
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", access_token))
            .context("Failed to create authorization header")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    
    // Create the request body for Gemini Pro 2 model with Google Search grounding
    let request_body = json!({
        "contents": [
            {
                "role": "user",
                "parts": [
                    {
                        "text": "Who is Hamze Ghalebi? Please provide information about their background, achievements, and contributions. Include details about their role as CTO at Remolab, their work with Welcome Place, and their expertise in technology and entrepreneurship."
                    }
                ]
            }
        ],
        "tools": [
            {
                "google_search_retrieval": {}
            }
        ],
        "generationConfig": {
            "temperature": 0.7,
            "maxOutputTokens": 1024,
            "topK": 40,
            "topP": 0.95
        }
    });
    
    // Make the API request
    let response = client
        .post(api_url)
        .headers(headers)
        .json(&request_body)
        .send()
        .context("Failed to make Vertex AI API request")?;
    
    // Check if the request was successful
    let status = response.status();
    if status.is_success() {
        let response_json: Value = response
            .json()
            .context("Failed to parse API response as JSON")?;
        
        // Extract and print the generated text
        if let Some(candidates) = response_json["candidates"].as_array() {
            if let Some(content) = candidates.first()
                .and_then(|c| c["content"]["parts"].as_array())
                .and_then(|parts| parts.first())
                .and_then(|p| p["text"].as_str()) {
                println!("\n{}", "✅ API test successful! Generated text:".green());
                println!("{}", content);
                
                // Print grounding metadata if available
                if let Some(grounding_metadata) = candidates.first()
                    .and_then(|c| c["groundingMetadata"].as_object()) {
                    println!("\n{}", "Grounding Sources:".yellow().bold());
                    if let Some(sources) = grounding_metadata.get("webSearchRetrievalResults") {
                        println!("{}", sources);
                    }
                }
            } else {
                println!("⚠️ Received valid API response but couldn't extract generated text.");
                println!("Raw response: {}", response_json);
            }
        } else {
            println!("⚠️ Received valid API response but couldn't find candidates.");
            println!("Raw response: {}", response_json);
        }
    } else {
        // If the request failed, print the error response
        let error_text = response.text().unwrap_or_else(|_| "Unable to get error details".to_string());
        return Err(anyhow::anyhow!(
            "API request failed with status code {}: {}",
            status,
            error_text
        ));
    }
    
    Ok(())
}

/// Gets an access token for API authentication
///
/// This function retrieves an access token for authenticating with
/// Google Cloud APIs using the gcloud auth print-access-token command.
fn get_access_token() -> Result<String> {
    let output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .context("Failed to execute gcloud auth print-access-token command")?;

    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Failed to get access token: {}",
            error_message
        ));
    }

    let access_token = String::from_utf8(output.stdout)
        .context("Failed to parse access token")?
        .trim()
        .to_string();

    if access_token.is_empty() {
        return Err(anyhow::anyhow!("Empty access token received. Please make sure you are authenticated with gcloud."));
    }

    Ok(access_token)
}

/// Prints instructions for using the authentication in future API calls
///
/// This function prints instructions for obtaining and using access tokens
/// for Vertex AI API calls in various programming languages.
fn print_authentication_instructions() {
    println!("\n{}", "Step 5: Instructions for future API usage".blue().bold());
    println!("{}", "=========================================".blue());
    
    println!("\n{}", "Authentication".yellow().bold());
    println!("For command-line usage, authenticate using:");
    println!("  {}", "gcloud auth login".cyan());
    println!("  {}", "gcloud auth application-default login".cyan());
    
    println!("\n{}", "Getting an access token for API calls:".yellow().bold());
    println!("In shell scripts:");
    println!("  {}", "ACCESS_TOKEN=$(gcloud auth print-access-token)".cyan());
    
    println!("\nIn Python:");
    println!("{}", r#"
# Using Google Cloud client libraries (recommended)
from google.cloud import aiplatform

# Initialize the Vertex AI SDK
aiplatform.init(project='YOUR_PROJECT_ID', location='us-central1')

# Alternatively, for direct REST API calls:
import subprocess
import requests

def get_access_token():
    result = subprocess.run(
        ['gcloud', 'auth', 'print-access-token'], 
        stdout=subprocess.PIPE, 
        text=True, 
        check=True
    )
    return result.stdout.strip()

# Make API calls
headers = {
    'Authorization': f'Bearer {get_access_token()}',
    'Content-Type': 'application/json'
}
    "#.cyan());
    
    println!("\nIn Rust:");
    println!("{}", r#"
// For direct API calls using reqwest
use std::process::Command;

fn get_access_token() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()?;
        
    if !output.status.success() {
        return Err("Failed to get access token".into());
    }
    
    let token = String::from_utf8(output.stdout)?.trim().to_string();
    Ok(token)
}

// Then in your API call function:
let access_token = get_access_token()?;
let client = reqwest::blocking::Client::new();
let response = client
    .post(api_url)
    .header("Authorization", format!("Bearer {}", access_token))
    .header("Content-Type", "application/json")
    .json(&request_body)
    .send()?;
    "#.cyan());

    println!("\n{}", "Vertex AI Endpoints".yellow().bold());
    println!("For Gemini Pro 2 model:");
    println!("  {}", "https://us-central1-aiplatform.googleapis.com/v1/projects/PROJECT_ID/locations/us-central1/publishers/google/models/gemini-1.5-pro:generateContent".cyan());
    
    println!("\nReplace {} with your actual project ID.", "PROJECT_ID".yellow());
    println!("\nFor more information, visit the Vertex AI documentation:");
    println!("  {}", "https://cloud.google.com/vertex-ai/docs".cyan());
}

/// Runs a command and prints its output if verbose is true
///
/// This function executes a command and returns its output.
/// If verbose is true, it also prints the command output to stdout.
#[allow(dead_code)]
fn run_command(command: &mut Command, verbose: bool) -> Result<Output> {
    let output = command.output().context("Failed to execute command")?;
    
    if verbose {
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
    }
    
    Ok(output)
}
