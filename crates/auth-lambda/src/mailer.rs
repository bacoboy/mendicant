use anyhow::{Context, Result};
use aws_sdk_sesv2::types::{Body, Content, Destination, EmailContent, Message};

/// Email sending abstraction — SES in production, stdout in local dev.
///
/// Set SES_FROM_ADDRESS and SES_REGION to use SES.
/// When absent, verification links are logged via tracing (local dev).
#[derive(Clone)]
pub enum Mailer {
    Ses(SesMailer),
    Stdout,
}

#[derive(Clone)]
pub struct SesMailer {
    pub client: aws_sdk_sesv2::Client,
    pub from_address: String,
}

impl Mailer {
    pub async fn from_env(aws_config: &aws_config::SdkConfig) -> Result<Self> {
        let Ok(from_address) = std::env::var("SES_FROM_ADDRESS") else {
            return Ok(Self::Stdout);
        };

        // SES is only configured in us-east-2. Override the region so Lambdas
        // in other regions still reach the correct SES endpoint.
        let region = std::env::var("SES_REGION").context("SES_REGION must be set when SES_FROM_ADDRESS is set")?;
        let ses_config = aws_sdk_sesv2::config::Builder::from(aws_config)
            .region(aws_sdk_sesv2::config::Region::new(region))
            .build();
        let client = aws_sdk_sesv2::Client::from_conf(ses_config);

        Ok(Self::Ses(SesMailer { client, from_address }))
    }

    pub async fn send_verification(&self, to: &str, verification_url: &str) -> Result<()> {
        match self {
            Self::Ses(ses) => ses.send_verification(to, verification_url).await,
            Self::Stdout => {
                tracing::info!(email = to, url = verification_url, "email verification link");
                Ok(())
            }
        }
    }
}

impl SesMailer {
    async fn send_verification(&self, to: &str, verification_url: &str) -> Result<()> {
        let subject = Content::builder()
            .data("Verify your Mendicant email address")
            .charset("UTF-8")
            .build()
            .context("failed to build subject")?;

        let body_text = Content::builder()
            .data(format!(
                "Click the link below to verify your email address and complete registration:\n\n{verification_url}\n\nThis link expires in 15 minutes."
            ))
            .charset("UTF-8")
            .build()
            .context("failed to build body")?;

        let body = Body::builder().text(body_text).build();
        let message = Message::builder().subject(subject).body(body).build();
        let content = EmailContent::builder().simple(message).build();

        self.client
            .send_email()
            .from_email_address(&self.from_address)
            .destination(Destination::builder().to_addresses(to).build())
            .content(content)
            .send()
            .await
            .context("SES SendEmail failed")?;

        Ok(())
    }
}
