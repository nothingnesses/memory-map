use crate::Config;
use lettre::{
	AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
	transport::smtp::authentication::Credentials,
};

pub async fn send_password_reset_email(
	config: &Config,
	to_email: &str,
	token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
	let email = Message::builder()
		.from(config.smtp_from.parse()?)
		.to(to_email.parse()?)
		.subject("Password Reset Request")
		.header(ContentType::TEXT_PLAIN)
		.body(format!(
			"Click the link below to reset your password:\n\n{}/reset-password?token={}\n\nThis link expires in 10 minutes.",
			config.frontend_url, token
		))?;

	let creds = Credentials::new(config.smtp_user.clone(), config.smtp_pass.clone());

	let mailer =
		AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)?.credentials(creds).build();

	mailer.send(email).await?;

	Ok(())
}
