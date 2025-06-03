use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_secretsmanager::Client;
use clap::Parser;
use vrenv::{EnvFileConfig, VrEnv};

#[derive(Parser)]
#[command(name = "vrenv")]
#[command(about = "A CLI tool to fetch AWS secrets and create environment files")]
struct Cli {
    /// AWS Secret ARN
    #[arg(help = "The ARN of the AWS secret to fetch")]
    secret_arn: String,

    /// Output directory (defaults to /var/run)
    #[arg(short, long, default_value = "/var/run")]
    output_dir: String,

    /// Custom name for the env file (defaults to extracting from ARN)
    #[arg(short, long)]
    name: Option<String>,

    /// AWS region
    #[arg(short, long, default_value = "us-west-2")]
    region: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize AWS config with specified region
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(cli.region))
        .load()
        .await;

    let client = Client::new(&config);
    let vrenv = VrEnv::new(client);

    // Create configuration
    let env_config = EnvFileConfig {
        secret_arn: cli.secret_arn.clone(),
        output_dir: cli.output_dir,
        file_name: cli.name,
    };

    // Fetch the secret and create environment file
    println!("Fetching secret: {}", cli.secret_arn);
    let env_file_path = vrenv.create_env_file_from_secret(env_config).await?;

    println!("Environment file created: {}", env_file_path);
    Ok(())
}
