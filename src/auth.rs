use anyhow::{Context, Result};
use std::process::Command;

/// Gets an access token for API authentication
///
/// This function retrieves an access token for authenticating with
/// Google Cloud APIs using the gcloud auth print-access-token command.
pub fn get_access_token() -> Result<String> {
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
        return Err(anyhow::anyhow!(
            "Empty access token received. Please make sure you are authenticated with gcloud."
        ));
    }

    Ok(access_token)
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
