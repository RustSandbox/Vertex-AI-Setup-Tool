use anyhow::{Context, Result};
use std::process::Command;

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
