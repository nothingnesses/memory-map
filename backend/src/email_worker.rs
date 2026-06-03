use {
	crate::{
		Config,
		db::queries::{
			CLAIM_EMAIL_OUTBOX_QUERY,
			DELETE_EMAIL_OUTBOX_QUERY,
			INSERT_EMAIL_OUTBOX_QUERY,
			MARK_EMAIL_OUTBOX_FAILED_QUERY,
		},
		email::send_password_reset_email,
		errors::AppError,
		worker::MaintenanceTask,
	},
	anyhow::Context,
	deadpool::managed::Pool,
	deadpool_postgres::{
		Client,
		Manager,
	},
	serde::{
		Deserialize,
		Serialize,
	},
	std::time::Duration,
	tokio_postgres::{
		Row,
		Transaction,
	},
};

pub const PASSWORD_RESET_EMAIL_KIND: &str = "password_reset";

#[derive(Clone, Debug, Deserialize)]
pub struct EmailOutboxConfig {
	#[serde(default = "EmailOutboxConfig::default_retry_seconds")]
	pub retry_seconds: i64,
	#[serde(default = "EmailOutboxConfig::default_lease_seconds")]
	pub lease_seconds: i64,
	#[serde(default = "EmailOutboxConfig::default_worker_interval_seconds")]
	pub worker_interval_seconds: i64,
	#[serde(default = "EmailOutboxConfig::default_batch_size")]
	pub batch_size: i64,
	#[serde(default = "EmailOutboxConfig::default_max_attempts")]
	pub max_attempts: i32,
}

impl EmailOutboxConfig {
	pub const fn default_retry_seconds() -> i64 {
		60
	}

	pub const fn default_lease_seconds() -> i64 {
		300
	}

	pub const fn default_worker_interval_seconds() -> i64 {
		30
	}

	pub const fn default_batch_size() -> i64 {
		100
	}

	pub const fn default_max_attempts() -> i32 {
		10
	}

	pub fn validate(&self) -> anyhow::Result<()> {
		if self.retry_seconds <= 0 {
			anyhow::bail!("email_outbox.retry_seconds must be greater than 0");
		}
		if self.lease_seconds <= 0 {
			anyhow::bail!("email_outbox.lease_seconds must be greater than 0");
		}
		if self.worker_interval_seconds <= 0 {
			anyhow::bail!("email_outbox.worker_interval_seconds must be greater than 0");
		}
		if self.batch_size <= 0 {
			anyhow::bail!("email_outbox.batch_size must be greater than 0");
		}
		if self.max_attempts <= 0 {
			anyhow::bail!("email_outbox.max_attempts must be greater than 0");
		}
		Ok(())
	}

	fn worker_interval(&self) -> Duration {
		Duration::from_secs(self.worker_interval_seconds as u64)
	}
}

impl Default for EmailOutboxConfig {
	fn default() -> Self {
		Self {
			retry_seconds: Self::default_retry_seconds(),
			lease_seconds: Self::default_lease_seconds(),
			worker_interval_seconds: Self::default_worker_interval_seconds(),
			batch_size: Self::default_batch_size(),
			max_attempts: Self::default_max_attempts(),
		}
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PasswordResetEmailPayload {
	pub email: String,
	pub token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailOutboxMessage {
	pub id: i64,
	pub kind: String,
	pub payload: String,
}

impl TryFrom<Row> for EmailOutboxMessage {
	type Error = AppError;

	fn try_from(row: Row) -> Result<Self, Self::Error> {
		Ok(Self {
			id: row.try_get("id").context("Failed to read email outbox id")?,
			kind: row.try_get("kind").context("Failed to read email outbox kind")?,
			payload: row.try_get("payload").context("Failed to read email outbox payload")?,
		})
	}
}

pub async fn enqueue_password_reset_email(
	transaction: &Transaction<'_>,
	email: &str,
	token: &str,
) -> Result<(), AppError> {
	let payload = serde_json::to_string(&PasswordResetEmailPayload {
		email: email.to_string(),
		token: token.to_string(),
	})
	.context("Failed to serialize password reset email payload")?;
	transaction
		.execute(INSERT_EMAIL_OUTBOX_QUERY, &[&PASSWORD_RESET_EMAIL_KIND, &payload])
		.await
		.context("Failed to enqueue password reset email")?;
	Ok(())
}

#[derive(Clone)]
pub struct EmailWorker {
	pool: Pool<Manager>,
	sender: SmtpEmailSender,
	config: EmailOutboxConfig,
}

impl EmailWorker {
	pub fn new(
		pool: Pool<Manager>,
		config: Config,
	) -> Self {
		Self {
			pool,
			config: config.email_outbox.clone(),
			sender: SmtpEmailSender {
				config,
			},
		}
	}
}

impl MaintenanceTask for EmailWorker {
	fn name(&self) -> &'static str {
		"email_outbox"
	}

	fn interval(&self) -> Duration {
		self.config.worker_interval()
	}

	async fn run_once(&self) -> Result<(), AppError> {
		let mut client = self.pool.get().await?;
		drain_email_outbox(&mut client, &self.sender, &self.config).await
	}
}

#[derive(Clone)]
struct SmtpEmailSender {
	config: Config,
}

trait EmailSender {
	async fn send_password_reset(
		&self,
		payload: &PasswordResetEmailPayload,
	) -> anyhow::Result<()>;
}

impl EmailSender for SmtpEmailSender {
	async fn send_password_reset(
		&self,
		payload: &PasswordResetEmailPayload,
	) -> anyhow::Result<()> {
		send_password_reset_email(&self.config, &payload.email, &payload.token).await
	}
}

trait EmailOutbox {
	async fn claim_email_outbox(
		&mut self,
		limit: i64,
		lease_seconds: i64,
		max_attempts: i32,
	) -> Result<Vec<EmailOutboxMessage>, AppError>;

	async fn clear_email_outbox(
		&mut self,
		ids: &[i64],
	) -> Result<(), AppError>;

	async fn mark_email_outbox_failed(
		&mut self,
		ids: &[i64],
		error_message: &str,
		retry_after_seconds: i64,
	) -> Result<(), AppError>;
}

impl EmailOutbox for Client {
	async fn claim_email_outbox(
		&mut self,
		limit: i64,
		lease_seconds: i64,
		max_attempts: i32,
	) -> Result<Vec<EmailOutboxMessage>, AppError> {
		let rows = self
			.query(CLAIM_EMAIL_OUTBOX_QUERY, &[&limit, &lease_seconds, &max_attempts])
			.await
			.context("Failed to claim email outbox rows")?;
		rows.into_iter().map(EmailOutboxMessage::try_from).collect()
	}

	async fn clear_email_outbox(
		&mut self,
		ids: &[i64],
	) -> Result<(), AppError> {
		if ids.is_empty() {
			return Ok(());
		}
		self.execute(DELETE_EMAIL_OUTBOX_QUERY, &[&ids])
			.await
			.context("Failed to clear delivered email outbox rows")?;
		Ok(())
	}

	async fn mark_email_outbox_failed(
		&mut self,
		ids: &[i64],
		error_message: &str,
		retry_after_seconds: i64,
	) -> Result<(), AppError> {
		if ids.is_empty() {
			return Ok(());
		}
		self.execute(MARK_EMAIL_OUTBOX_FAILED_QUERY, &[&ids, &error_message, &retry_after_seconds])
			.await
			.context("Failed to record email outbox delivery failure")?;
		Ok(())
	}
}

async fn drain_email_outbox(
	outbox: &mut impl EmailOutbox,
	sender: &impl EmailSender,
	config: &EmailOutboxConfig,
) -> Result<(), AppError> {
	let mut first_error: Option<AppError> = None;
	loop {
		let messages = outbox
			.claim_email_outbox(config.batch_size, config.lease_seconds, config.max_attempts)
			.await?;

		if messages.is_empty() {
			break;
		}

		for message in messages {
			if let Err(error) = send_email_message(sender, &message).await {
				let error_message = error.to_string();
				outbox
					.mark_email_outbox_failed(&[message.id], &error_message, config.retry_seconds)
					.await?;
				if first_error.is_none() {
					first_error = Some(AppError::Internal(error));
				}
				continue;
			}

			outbox.clear_email_outbox(&[message.id]).await?;
		}
	}

	match first_error {
		Some(error) => Err(error),
		None => Ok(()),
	}
}

async fn send_email_message(
	sender: &impl EmailSender,
	message: &EmailOutboxMessage,
) -> anyhow::Result<()> {
	match message.kind.as_str() {
		PASSWORD_RESET_EMAIL_KIND => {
			let payload = serde_json::from_str::<PasswordResetEmailPayload>(&message.payload)
				.context("Failed to deserialize password reset email payload")?;
			sender.send_password_reset(&payload).await
		}
		kind => anyhow::bail!("Unsupported email outbox kind: {kind}"),
	}
}

#[cfg(test)]
mod tests {
	use {
		super::{
			EmailOutbox,
			EmailOutboxConfig,
			EmailOutboxMessage,
			EmailSender,
			PASSWORD_RESET_EMAIL_KIND,
			PasswordResetEmailPayload,
			drain_email_outbox,
		},
		std::sync::Mutex,
	};

	#[derive(Default)]
	struct FakeOutbox {
		claims: Vec<Vec<EmailOutboxMessage>>,
		cleared: Vec<i64>,
		failed: Vec<(Vec<i64>, String, i64)>,
	}

	impl EmailOutbox for FakeOutbox {
		async fn claim_email_outbox(
			&mut self,
			_limit: i64,
			_lease_seconds: i64,
			_max_attempts: i32,
		) -> Result<Vec<EmailOutboxMessage>, crate::errors::AppError> {
			if self.claims.is_empty() {
				return Ok(Vec::new());
			}
			Ok(self.claims.remove(0))
		}

		async fn clear_email_outbox(
			&mut self,
			ids: &[i64],
		) -> Result<(), crate::errors::AppError> {
			self.cleared.extend_from_slice(ids);
			Ok(())
		}

		async fn mark_email_outbox_failed(
			&mut self,
			ids: &[i64],
			error_message: &str,
			retry_after_seconds: i64,
		) -> Result<(), crate::errors::AppError> {
			self.failed.push((ids.to_vec(), error_message.to_string(), retry_after_seconds));
			Ok(())
		}
	}

	#[derive(Default)]
	struct FakeEmailSender {
		sent: Mutex<Vec<PasswordResetEmailPayload>>,
		fail: bool,
	}

	impl FakeEmailSender {
		fn sent(&self) -> anyhow::Result<Vec<PasswordResetEmailPayload>> {
			self.sent
				.lock()
				.map(|sent| sent.clone())
				.map_err(|_| anyhow::anyhow!("sent mutex poisoned"))
		}
	}

	impl EmailSender for FakeEmailSender {
		async fn send_password_reset(
			&self,
			payload: &PasswordResetEmailPayload,
		) -> anyhow::Result<()> {
			if self.fail {
				anyhow::bail!("smtp failed");
			}
			self.sent
				.lock()
				.map_err(|_| anyhow::anyhow!("sent mutex poisoned"))?
				.push(payload.clone());
			Ok(())
		}
	}

	#[tokio::test]
	async fn drain_email_outbox_sends_and_clears_password_reset_rows() -> anyhow::Result<()> {
		let payload = PasswordResetEmailPayload {
			email: "person@example.test".to_string(),
			token: "reset-token".to_string(),
		};
		let mut outbox = FakeOutbox {
			claims: vec![vec![EmailOutboxMessage {
				id: 42,
				kind: PASSWORD_RESET_EMAIL_KIND.to_string(),
				payload: serde_json::to_string(&payload)?,
			}]],
			..FakeOutbox::default()
		};
		let sender = FakeEmailSender::default();

		drain_email_outbox(&mut outbox, &sender, &EmailOutboxConfig::default()).await?;

		assert_eq!(sender.sent()?, vec![payload]);
		assert_eq!(outbox.cleared, vec![42]);
		assert!(outbox.failed.is_empty());
		Ok(())
	}

	#[tokio::test]
	async fn drain_email_outbox_records_send_failures() -> anyhow::Result<()> {
		let payload = PasswordResetEmailPayload {
			email: "person@example.test".to_string(),
			token: "reset-token".to_string(),
		};
		let mut outbox = FakeOutbox {
			claims: vec![vec![EmailOutboxMessage {
				id: 42,
				kind: PASSWORD_RESET_EMAIL_KIND.to_string(),
				payload: serde_json::to_string(&payload)?,
			}]],
			..FakeOutbox::default()
		};
		let sender = FakeEmailSender {
			fail: true,
			..FakeEmailSender::default()
		};
		let config = EmailOutboxConfig {
			retry_seconds: 17,
			..EmailOutboxConfig::default()
		};

		let Err(error) = drain_email_outbox(&mut outbox, &sender, &config).await else {
			anyhow::bail!("drain unexpectedly succeeded");
		};

		assert!(error.to_string().contains("smtp failed"));
		assert!(outbox.cleared.is_empty());
		assert_eq!(outbox.failed, vec![(vec![42], "smtp failed".to_string(), 17)]);
		Ok(())
	}
}
