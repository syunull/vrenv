use anyhow::{Context, Result};
use aws_sdk_secretsmanager::Client;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Configuration for creating environment files from AWS secrets
pub struct EnvFileConfig {
    pub secret_arn: String,
    pub output_dir: String,
    pub file_name: Option<String>,
}

/// Main service for handling AWS secrets and environment file creation
pub struct VrEnv {
    client: Client,
}

impl VrEnv {
    /// Create a new SecretEnvService with the provided AWS client
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetch a secret from AWS Secrets Manager and create an environment file
    pub async fn create_env_file_from_secret(&self, config: EnvFileConfig) -> Result<String> {
        // Fetch the secret
        let secret_value = self.fetch_secret(&config.secret_arn).await?;

        // Extract name from ARN if not provided
        let env_file_name = config
            .file_name
            .unwrap_or_else(|| extract_secret_name_from_arn(&config.secret_arn));

        // Create the environment file
        let output_path = Path::new(&config.output_dir);
        let env_file_path = output_path.join(format!("{}.env", env_file_name));
        create_env_file(&secret_value, &env_file_path)?;

        Ok(env_file_path.to_string_lossy().to_string())
    }

    /// Fetch a secret value from AWS Secrets Manager
    pub async fn fetch_secret(&self, secret_arn: &str) -> Result<String> {
        let response = self
            .client
            .get_secret_value()
            .secret_id(secret_arn)
            .send()
            .await
            .context("Failed to fetch secret from AWS")?;

        response
            .secret_string()
            .context("Secret does not contain a string value")
            .map(|s| s.to_string())
    }
}

/// Extract the secret name from an AWS Secrets Manager ARN
pub fn extract_secret_name_from_arn(arn: &str) -> String {
    // Extract the secret name from ARN format: arn:aws:secretsmanager:region:account:secret:name-suffix
    arn.split(':')
        .nth(6)
        .and_then(|name_with_suffix| name_with_suffix.split('-').next())
        .unwrap_or("secret")
        .split('/')  // Handle paths in secret names
        .last()      // Take only the last part after the final slash
        .unwrap_or("secret")
        .to_string()
}

/// Create an environment file from a secret value
pub fn create_env_file(secret_value: &str, file_path: &Path) -> Result<()> {
    // Ensure the output directory exists
    if let Some(parent) = file_path.parent() {
        println!("Creating output directory: {}", parent.display());
        fs::create_dir_all(parent).context("Failed to create output directory")?;
    }

    // Parse the secret value as JSON and convert to environment variables
    let env_content = if let Ok(json_value) = serde_json::from_str::<Value>(secret_value) {
        json_to_env_format(&json_value)?
    } else {
        // If it's not JSON, treat it as a single value
        format!("SECRET_VALUE={}\n", secret_value)
    };

    // Write to file
    fs::write(file_path, env_content).context("Failed to write environment file")?;

    // Set appropriate permissions (readable by owner and group)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(file_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(file_path, perms)?;
    }

    Ok(())
}

/// Convert a JSON value to environment variable format
pub fn json_to_env_format(json_value: &Value) -> Result<String> {
    let mut env_lines = Vec::new();

    match json_value {
        Value::Object(map) => {
            for (key, value) in map {
                let env_key = key.to_uppercase().replace(['-', ' '], "_");
                let env_value = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => String::new(),
                    _ => serde_json::to_string(value)
                        .context("Failed to serialize complex JSON value")?,
                };
                env_lines.push(format!("{}={}", env_key, env_value));
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Secret value must be a JSON object with key-value pairs"
            ));
        }
    }

    env_lines.sort();
    Ok(env_lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_secret_name_from_arn() {
        let arn = "arn:aws:secretsmanager:us-west-2:123456789012:secret:MySecret-AbCdEf";
        assert_eq!(extract_secret_name_from_arn(arn), "MySecret");

        // Test with path-like secret name
        let arn_with_path = "arn:aws:secretsmanager:us-west-2:123456789012:secret:/my/custom/path/secret-AbCdEf";
        assert_eq!(extract_secret_name_from_arn(arn_with_path), "secret");
    }

    #[test]
    fn test_json_to_env_format() {
        let json_str = r#"{"database_url": "postgres://localhost", "api_key": "secret123"}"#;
        let json_value: Value = serde_json::from_str(json_str).unwrap();
        let result = json_to_env_format(&json_value).unwrap();

        assert!(result.contains("API_KEY=secret123"));
        assert!(result.contains("DATABASE_URL=postgres://localhost"));
    }

    #[test]
    fn test_json_to_env_format_with_different_types() {
        let json_str = r#"{"port": 8080, "debug": true, "timeout": null}"#;
        let json_value: Value = serde_json::from_str(json_str).unwrap();
        let result = json_to_env_format(&json_value).unwrap();

        assert!(result.contains("DEBUG=true"));
        assert!(result.contains("PORT=8080"));
        assert!(result.contains("TIMEOUT="));
    }
}
