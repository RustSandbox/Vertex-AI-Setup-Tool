mod auth;
mod models;
mod pdf;
mod queue;
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
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use regex::Regex;
use serde_json::Value;
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::Semaphore, time::sleep};

use crate::auth::get_access_token;
use crate::models::list_vertex_ai_models;
use crate::pdf::extract_data_from_pdf_v2;
use crate::queue::{QueueConfig, RequestQueue};
use crate::setup::{ensure_vertex_ai_service, test_vertex_ai_api_call};

/// Maximum number of retries for rate-limited requests
const MAX_RETRIES: u32 = 3;
/// Base delay for exponential backoff (in milliseconds)
const BASE_DELAY_MS: u64 = 1000;
/// Maximum concurrent PDF processing tasks
const MAX_CONCURRENT_TASKS: usize = 3;

/// Struct to hold logging information
#[derive(Debug)]
struct ExtractionLog {
    timestamp: String,
    file_path: String,
    status: String,
    error_message: Option<String>,
}

impl ExtractionLog {
    fn new(file_path: String, status: String, error_message: Option<String>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        Self {
            timestamp,
            file_path,
            status,
            error_message,
        }
    }

    fn to_csv_line(&self) -> String {
        format!(
            "{},{},{},{}\n",
            self.timestamp,
            self.file_path,
            self.status,
            self.error_message.as_deref().unwrap_or("-")
        )
    }
}

/// Writes a log entry to the appropriate log file
fn write_log_entry(log_dir: &Path, log: ExtractionLog) -> Result<()> {
    let file_name = if log.status == "SUCCESS" {
        "successful_extractions.csv"
    } else {
        "failed_extractions.csv"
    };

    let log_file_path = log_dir.join(file_name);

    // Create the file if it doesn't exist and write headers
    if !log_file_path.exists() {
        let mut file = fs::File::create(&log_file_path)?;
        file.write_all(b"timestamp,file_path,status,error_message\n")?;
    }

    // Append the log entry
    fs::OpenOptions::new()
        .append(true)
        .open(log_file_path)?
        .write_all(log.to_csv_line().as_bytes())?;

    Ok(())
}

/// Processes a single PDF file asynchronously with retry logic
///
/// # Arguments
///
/// * `path` - Path to the PDF file
/// * `input_dir` - Base input directory for calculating relative paths
/// * `output_base_dir` - Base output directory for saving JSON files
/// * `log_dir` - Base log directory for saving extraction logs
/// * `request_queue` - Request queue for rate limiting
/// * `progress_bar` - Progress bar for tracking progress
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Success or error status
async fn process_single_pdf(
    path: PathBuf,
    input_dir: &Path,
    output_base_dir: &Path,
    log_dir: &Path,
    request_queue: &RequestQueue,
    progress_bar: ProgressBar,
) -> Result<()> {
    // Set the progress bar style
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Update progress message with the current file
    progress_bar.set_message(format!("Processing: {}", path.display()));

    // Read and encode the PDF file
    let pdf_bytes = fs::read(&path)?;
    let pdf_base64 = general_purpose::STANDARD.encode(pdf_bytes);

    // Create the output directory structure
    let relative_path = path.parent().unwrap().strip_prefix(input_dir)?;
    let output_dir = output_base_dir.join(relative_path);
    fs::create_dir_all(&output_dir)?;

    // Generate output filename
    let output_filename = path.file_stem().unwrap().to_string_lossy().to_string() + ".json";
    let output_path = output_dir.join(output_filename);

    // Clone values for the closure
    let pdf_base64 = pdf_base64.clone();
    let path_display = path.display().to_string();

    // Execute the request through the queue
    match request_queue
        .execute(move || {
            // This closure will be retried automatically by the queue system
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    extract_data_from_pdf_v2(&pdf_base64, None, None, None, None, None).await
                })
            })
        })
        .await
    {
        Ok(api_response) => {
            // Process the response
            let json_data =
                if let Some(raw_text) = api_response.get("raw_text").and_then(|v| v.as_str()) {
                    match extract_json_from_raw_text(raw_text) {
                        Ok(extracted_json) => extracted_json,
                        Err(e) => {
                            progress_bar.set_message(format!(
                                "Warning: Failed to extract JSON from {}: {}",
                                path.display(),
                                e
                            ));
                            api_response
                        }
                    }
                } else {
                    api_response
                };

            // Write the JSON to file
            let json_str = serde_json::to_string_pretty(&json_data)?;
            fs::write(&output_path, json_str)?;

            // Log successful extraction
            let log = ExtractionLog::new(path_display.clone(), "SUCCESS".to_string(), None);
            write_log_entry(log_dir, log)?;

            // Update progress bar
            progress_bar.finish_with_message(format!("✅ Completed: {}", path_display));
            Ok(())
        }
        Err(e) => {
            // Log the error
            let log = ExtractionLog::new(
                path_display.clone(),
                "FAILED".to_string(),
                Some(e.to_string()),
            );
            write_log_entry(log_dir, log)?;

            // Update progress bar with error
            progress_bar.finish_with_message(format!("❌ Failed: {} - {}", path_display, e));
            Err(e)
        }
    }
}

/// Collects all PDF files from a directory recursively
///
/// # Arguments
///
/// * `dir` - Directory to scan for PDF files
///
/// # Returns
///
/// * `Result<Vec<PathBuf>, anyhow::Error>` - List of PDF file paths
fn collect_pdf_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut pdf_files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            pdf_files.extend(collect_pdf_files(&path)?);
        } else if let Some(extension) = path.extension() {
            if extension.to_string_lossy().to_lowercase() == "pdf" {
                pdf_files.push(path);
            }
        }
    }

    Ok(pdf_files)
}

/// Processes all PDF files in a directory recursively and asynchronously
///
/// # Arguments
///
/// * `input_dir` - The input directory containing PDF files
/// * `output_base_dir` - The base directory where extracted JSON files will be saved
/// * `log_dir` - The base directory where extraction logs will be saved
///
/// # Returns
///
/// * `Result<(), anyhow::Error>` - Success or error status
async fn process_pdfs_recursively(
    input_dir: &Path,
    output_base_dir: &Path,
    log_dir: &Path,
) -> Result<()> {
    // Create the output and log directories if they don't exist
    fs::create_dir_all(output_base_dir)?;
    fs::create_dir_all(log_dir)?;

    // Collect all PDF files first
    let pdf_files = collect_pdf_files(input_dir)?;
    let total_files = pdf_files.len();
    println!("\nFound {} PDF files to process", total_files);

    // Create the request queue with custom configuration
    let queue_config = QueueConfig {
        max_tokens: 1000000,                      // 1 million tokens to handle large PDFs
        refill_tokens: 100000,                    // Refill 100k tokens per interval
        refill_interval: Duration::from_secs(60), // Refill every minute
        max_concurrent_requests: MAX_CONCURRENT_TASKS,
    };
    let request_queue = Arc::new(RequestQueue::new(queue_config));

    println!("\n{}", "Queue Configuration:".blue().bold());
    println!("Max Tokens: {}", "1,000,000".cyan());
    println!("Refill Rate: {} tokens per minute", "100,000".cyan());
    println!(
        "Concurrent Tasks: {}\n",
        MAX_CONCURRENT_TASKS.to_string().cyan()
    );

    // Create multi-progress bar
    let multi_progress = Arc::new(MultiProgress::new());

    // Process files in parallel with controlled concurrency
    let mut tasks = futures::stream::iter(pdf_files.into_iter().map(|pdf_path| {
        let request_queue = Arc::clone(&request_queue);
        let input_dir = Arc::new(input_dir.to_path_buf());
        let output_base_dir = Arc::new(output_base_dir.to_path_buf());
        let log_dir = Arc::new(log_dir.to_path_buf());
        let multi_progress = Arc::clone(&multi_progress);

        async move {
            // Create a new progress bar for this file
            let progress_bar = multi_progress.add(ProgressBar::new(1));

            let result = process_single_pdf(
                pdf_path.clone(),
                &input_dir,
                &output_base_dir,
                &log_dir,
                &request_queue,
                progress_bar,
            )
            .await;

            if let Err(e) = result {
                eprintln!("Error processing {}: {}", pdf_path.display(), e);
            }
        }
    }))
    .buffer_unordered(MAX_CONCURRENT_TASKS)
    .collect::<Vec<_>>();

    // Wait for all tasks to complete
    tasks.await;

    Ok(())
}

/// Main entry point for the application
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Print a welcome message with styling
    println!("{}", "Vertex AI PDF Data Extraction Tool".green().bold());
    println!("{}", "================================".green());
    println!();

    // Load environment variables
    load_environment()?;

    // Check environment variables
    check_environment_variables()?;

    // Define input, output, and log directories
    let input_dir = PathBuf::from("/Users/hamzeghalebi/project/remolab/giva/pdfproject/data");
    let output_dir = input_dir.parent().unwrap().join("extracted_data");
    let log_dir = input_dir.parent().unwrap().join("logs");

    println!("\n{}", "Starting PDF processing...".blue().bold());
    println!(
        "Input directory: {}",
        input_dir.display().to_string().cyan()
    );
    println!(
        "Output directory: {}",
        output_dir.display().to_string().cyan()
    );
    println!("Log directory: {}", log_dir.display().to_string().cyan());

    // Process all PDFs recursively and asynchronously
    process_pdfs_recursively(&input_dir, &output_dir, &log_dir).await?;

    println!("\n{}", "Processing complete!".green().bold());
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
