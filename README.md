# Vertex AI Setup Tool

A Rust-based command-line tool for setting up and testing Google Cloud Vertex AI integration. This tool helps you:
- Enable Vertex AI service
- List available models
- Set up authentication
- Test API calls with Gemini Pro 2 model and Google Search grounding

## Prerequisites

- Rust 1.70 or later
- Google Cloud SDK installed
- A Google Cloud project with billing enabled
- Appropriate permissions to enable services and make API calls

## Installation

1. Clone the repository:
```bash
git clone https://github.com/yourusername/vertex-ai-setup.git
cd vertex-ai-setup
```

2. Build the project:
```bash
cargo build --release
```

## Usage

Run the tool:
```bash
cargo run --release
```

The tool will:
1. Check if Vertex AI is enabled and enable it if necessary
2. List available models using `gcloud ai models list`
3. Set up authentication using Application Default Credentials (ADC)
4. Test the API with a sample query using Gemini Pro 2 model with Google Search grounding
5. Display instructions for future API usage

## Features

- **Automatic Service Enablement**: Checks and enables Vertex AI service if needed
- **Model Discovery**: Lists available Vertex AI models in your project
- **Authentication Setup**: Configures Application Default Credentials (ADC)
- **API Testing**: Tests the Vertex AI API using Gemini Pro 2 model
- **Google Search Grounding**: Utilizes Google Search to enhance responses with real-time information
- **Comprehensive Documentation**: Provides detailed instructions for future API usage

## API Usage Examples

The tool provides examples for using the Vertex AI API in various programming languages:

### Python
```python
from google.cloud import aiplatform

# Initialize the Vertex AI SDK
aiplatform.init(project='YOUR_PROJECT_ID', location='us-central1')
```

### Rust
```rust
use std::process::Command;

fn get_access_token() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()?;
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
```

## Troubleshooting

1. If you encounter permission issues:
   - Ensure you have the necessary IAM roles (e.g., `roles/aiplatform.user`)
   - Run `gcloud auth login` and `gcloud auth application-default login`

2. If models are not listed:
   - Run `gcloud ai models list --region=us-central1` manually to verify access
   - Check if your project has billing enabled

3. If API calls fail:
   - Verify your authentication is set up correctly
   - Check if the Vertex AI service is enabled
   - Ensure your project has sufficient quota

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details. 