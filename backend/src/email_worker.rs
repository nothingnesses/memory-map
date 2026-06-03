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
		outbox::{
			DrainOutcome,
			FailedGroup,
			OutboxProcessor,
			OutboxQueue,
			OutboxRetryConfig,
			drain_outbox,
			ensure_positive,
		},
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
	std::{
		future::Future,
		time::Duration,
	},
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
		self.retry().validate("email_outbox")?;
		ensure_positive!(self, worker_interval_seconds);
		Ok(())
	}

	/// Lease/retry policy for the email outbox, as a runtime view.
	pub fn retry(&self) -> OutboxRetryConfig {
		OutboxRetryConfig {
			retry_seconds: self.retry_seconds,
			lease_seconds: self.lease_seconds,
			batch_size: self.batch_size,
			max_attempts: self.max_attempts,
		}
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
		let processor = EmailOutboxProcessor {
			sender: &self.sender,
		};
		let mut queue = EmailOutboxQueue(&mut client);
		drain_outbox(&mut queue, &processor, &self.config.retry()).await
	}
}

#[derive(Clone)]
struct SmtpEmailSender {
	config: Config,
}

/// Sends one email. `send_password_reset` carries an explicit `Send` bound on its
/// future so the generic `EmailOutboxProcessor` can compose it into the
/// `Send`-bounded `OutboxProcessor::process` future (`drain_outbox` runs in a
/// spawned worker task).
trait EmailSender {
	fn send_password_reset(
		&self,
		payload: &PasswordResetEmailPayload,
	) -> impl Future<Output = anyhow::Result<()>> + Send;
}

impl EmailSender for SmtpEmailSender {
	async fn send_password_reset(
		&self,
		payload: &PasswordResetEmailPayload,
	) -> anyhow::Result<()> {
		send_password_reset_email(&self.config, &payload.email, &payload.token).await
	}
}

/// The email outbox as an [`OutboxQueue`]. `clear`/`mark_failed` operate on the
/// message ids; the item carries the kind and payload the processor needs.
struct EmailOutboxQueue<'a>(&'a mut Client);

impl OutboxQueue for EmailOutboxQueue<'_> {
	type Item = EmailOutboxMessage;

	async fn claim(
		&mut self,
		retry: &OutboxRetryConfig,
	) -> Result<Vec<EmailOutboxMessage>, AppError> {
		let rows = self
			.0
			.query(
				CLAIM_EMAIL_OUTBOX_QUERY,
				&[&retry.batch_size, &retry.lease_seconds, &retry.max_attempts],
			)
			.await
			.context("Failed to claim email outbox rows")?;
		rows.into_iter().map(EmailOutboxMessage::try_from).collect()
	}

	async fn clear(
		&mut self,
		messages: &[EmailOutboxMessage],
	) -> Result<(), AppError> {
		if messages.is_empty() {
			return Ok(());
		}
		let ids = messages.iter().map(|message| message.id).collect::<Vec<_>>();
		self.0
			.execute(DELETE_EMAIL_OUTBOX_QUERY, &[&ids])
			.await
			.context("Failed to clear delivered email outbox rows")?;
		Ok(())
	}

	async fn mark_failed(
		&mut self,
		messages: &[EmailOutboxMessage],
		error_message: &str,
		retry_after_seconds: i64,
	) -> Result<(), AppError> {
		if messages.is_empty() {
			return Ok(());
		}
		let ids = messages.iter().map(|message| message.id).collect::<Vec<_>>();
		self.0
			.execute(MARK_EMAIL_OUTBOX_FAILED_QUERY, &[&ids, &error_message, &retry_after_seconds])
			.await
			.context("Failed to record email outbox delivery failure")?;
		Ok(())
	}
}

/// Sends each claimed message independently, so one failed send does not fail the
/// rest of the batch: successes clear, and each failure becomes its own
/// `FailedGroup` to be marked for retry.
struct EmailOutboxProcessor<'a, S> {
	sender: &'a S,
}

impl<S: EmailSender + Sync> OutboxProcessor for EmailOutboxProcessor<'_, S> {
	type Item = EmailOutboxMessage;

	async fn process(
		&self,
		messages: Vec<EmailOutboxMessage>,
	) -> DrainOutcome<EmailOutboxMessage> {
		let mut cleared = Vec::new();
		let mut failed = Vec::new();
		for message in messages {
			match send_email_message(self.sender, &message).await {
				Ok(()) => cleared.push(message),
				Err(error) => failed.push(FailedGroup {
					items: vec![message],
					error,
				}),
			}
		}
		DrainOutcome {
			cleared,
			failed,
		}
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
			EmailOutboxMessage,
			EmailOutboxProcessor,
			EmailSender,
			PASSWORD_RESET_EMAIL_KIND,
			PasswordResetEmailPayload,
		},
		crate::outbox::OutboxProcessor,
		std::sync::Mutex,
	};

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

	fn password_reset_message(id: i64) -> anyhow::Result<EmailOutboxMessage> {
		Ok(EmailOutboxMessage {
			id,
			kind: PASSWORD_RESET_EMAIL_KIND.to_string(),
			payload: serde_json::to_string(&PasswordResetEmailPayload {
				email: "person@example.test".to_string(),
				token: "reset-token".to_string(),
			})?,
		})
	}

	#[tokio::test]
	async fn email_processor_sends_and_clears_password_reset_rows() -> anyhow::Result<()> {
		let sender = FakeEmailSender::default();
		let processor = EmailOutboxProcessor {
			sender: &sender,
		};

		let outcome = processor.process(vec![password_reset_message(42)?]).await;

		assert_eq!(outcome.cleared.iter().map(|message| message.id).collect::<Vec<_>>(), vec![42]);
		assert!(outcome.failed.is_empty());
		assert_eq!(
			sender.sent()?,
			vec![PasswordResetEmailPayload {
				email: "person@example.test".to_string(),
				token: "reset-token".to_string(),
			}]
		);
		Ok(())
	}

	#[tokio::test]
	async fn email_processor_isolates_send_failures_per_message() -> anyhow::Result<()> {
		let sender = FakeEmailSender {
			fail: true,
			..FakeEmailSender::default()
		};
		let processor = EmailOutboxProcessor {
			sender: &sender,
		};

		let outcome = processor.process(vec![password_reset_message(42)?]).await;

		assert!(outcome.cleared.is_empty());
		let [group] = outcome.failed.as_slice() else {
			anyhow::bail!("expected exactly one failed group");
		};
		assert_eq!(group.items.iter().map(|message| message.id).collect::<Vec<_>>(), vec![42]);
		assert!(group.error.to_string().contains("smtp failed"));
		Ok(())
	}
}
