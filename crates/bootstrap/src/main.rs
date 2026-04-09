//! Admin bootstrap tool.
//!
//! Creates the first administrator user directly in DynamoDB and emits a
//! single-use YubiKey enrollment URL. Run once; re-run if the token expires
//! or enrollment fails (a new token is created each time, the user record is
//! reused if the email already has Role::Administrator).
//!
//! # Usage
//!
//!   cargo run -p bootstrap -- admin@example.com --site-url https://example.com
//!
//! # Required environment variables
//!
//!   TABLE_USERS, TABLE_CREDENTIALS, TABLE_REFRESH_TOKENS,
//!   TABLE_CHALLENGES, TABLE_EMAIL_TOKENS, TABLE_OAUTH_DEVICES
//!
//! For local dev these are set in docker-compose.yml. For AWS, set them to
//! match your Terraform outputs before running.
//!
//! Set DYNAMODB_ENDPOINT_URL=http://localhost:8000 to target DynamoDB Local.

use anyhow::{Context as _, bail};
use clap::Parser;
use db::challenges::ChallengeRepository;
use db::client::DynamoClient;
use db::users::UserRepository;
use domain::challenge::Challenge;
use domain::user::{Role, User, UserStatus};

const DEFAULT_TTL_MINUTES: u64 = 60;

#[derive(Parser)]
#[command(
    name = "bootstrap",
    about = "Create the first admin user and emit a YubiKey enrollment URL"
)]
struct Args {
    /// Email address for the administrator account
    email: String,

    /// Display name shown in the UI
    #[arg(long, default_value = "Administrator")]
    display_name: String,

    /// Base URL of the site (no trailing slash)
    #[arg(long, env = "SITE_URL", default_value = "https://localhost:9001")]
    site_url: String,

    /// How many minutes the enrollment URL remains valid
    #[arg(long, default_value_t = DEFAULT_TTL_MINUTES)]
    ttl_minutes: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;

    let ddb_client = if let Ok(endpoint) = std::env::var("DYNAMODB_ENDPOINT_URL") {
        let ddb_config = aws_sdk_dynamodb::config::Builder::from(&config)
            .endpoint_url(endpoint)
            .build();
        aws_sdk_dynamodb::Client::from_conf(ddb_config)
    } else {
        aws_sdk_dynamodb::Client::new(&config)
    };

    let db = DynamoClient::from_env(ddb_client);
    let users_repo = UserRepository::new(db.clone());
    let challenges_repo = ChallengeRepository::new(db.clone());

    // Resolve or create the admin user record.
    let user = match users_repo.get_by_email(&args.email).await {
        Ok(existing) => {
            if existing.role != Role::Administrator {
                bail!(
                    "An account for {} already exists but has role {:?}. \
                     Promote it manually via update_role if intended.",
                    args.email,
                    existing.role
                );
            }
            println!("Admin user already exists — reusing it.");
            existing
        }
        Err(_) => {
            let mut user = User::new(args.email.clone(), args.display_name.clone());
            user.role = Role::Administrator;
            user.status = UserStatus::Active;
            users_repo
                .put(&user)
                .await
                .context("failed to create admin user in DynamoDB")?;
            println!("Admin user created.");
            user
        }
    };

    // Issue a fresh enrollment token.
    let ttl_secs = args.ttl_minutes as i64 * 60;
    let expires_at = time::OffsetDateTime::now_utc().unix_timestamp() + ttl_secs;
    let token = Challenge::new_admin_enrollment(user.id.to_string(), expires_at);
    let token_id = token.id.clone();

    challenges_repo
        .put(&token)
        .await
        .context("failed to store enrollment token in DynamoDB")?;

    let site_url = args.site_url.trim_end_matches('/');

    println!();
    println!("  User ID : {}", user.id);
    println!("  Email   : {}", user.email);
    println!();
    println!("Enrollment URL (valid for {} minutes):", args.ttl_minutes);
    println!();
    println!("  {site_url}/admin/enroll?token={token_id}");
    println!();
    println!("Open the URL in a browser on the machine where your YubiKey is attached.");
    println!("The link is single-use. Re-run this tool to generate a new one.");

    Ok(())
}
