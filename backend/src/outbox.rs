//! Shared primitives for the transactional outbox / lease-retry pattern used by
//! the storage-deletion and email-outbox workers.

use {
	crate::errors::AppError,
	std::future::Future,
};

/// Bails with an `anyhow` error if a config field is not strictly positive,
/// naming the field in the message via `stringify!`. This removes the repetitive
/// per-field `if x <= 0 { bail!("x must be greater than 0") }` blocks (and the
/// copy-paste hazard of a check naming the wrong field).
macro_rules! ensure_positive {
	($obj:expr, $field:ident) => {
		if $obj.$field <= 0 {
			::anyhow::bail!("{} must be greater than 0", ::core::stringify!($field));
		}
	};
}
pub(crate) use ensure_positive;

/// Runtime view of one outbox's lease/retry policy.
///
/// This is deliberately NOT a deserialized config struct: each owning config
/// (storage deletion, upload-session cleanup, email outbox) keeps its own flat
/// fields so their genuinely different defaults stay correct (e.g. `batch_size`
/// defaults to 1000 for storage, matching S3's multi-delete cap, but 100 for
/// email). The owners build this view via accessors; the drain loop and claim
/// queries consume it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutboxRetryConfig {
	pub retry_seconds: i64,
	pub lease_seconds: i64,
	pub batch_size: i64,
	pub max_attempts: i32,
}

impl OutboxRetryConfig {
	/// Validates that every knob is strictly positive, prefixing the field name
	/// with `label` (the owning group, e.g. `storage_deletion`) so operator-facing
	/// errors stay specific.
	pub fn validate(
		&self,
		label: &str,
	) -> anyhow::Result<()> {
		if self.retry_seconds <= 0 {
			anyhow::bail!("{label}.retry_seconds must be greater than 0");
		}
		if self.lease_seconds <= 0 {
			anyhow::bail!("{label}.lease_seconds must be greater than 0");
		}
		if self.batch_size <= 0 {
			anyhow::bail!("{label}.batch_size must be greater than 0");
		}
		if self.max_attempts <= 0 {
			anyhow::bail!("{label}.max_attempts must be greater than 0");
		}
		Ok(())
	}
}

/// A claimed batch that failed processing, paired with the error to record and
/// surface.
pub struct FailedGroup<Item> {
	pub items: Vec<Item>,
	pub error: anyhow::Error,
}

/// The partition of one processed batch into items to clear (done) and groups to
/// mark failed (to retry later).
pub struct DrainOutcome<Item> {
	pub cleared: Vec<Item>,
	pub failed: Vec<FailedGroup<Item>>,
}

impl<Item> DrainOutcome<Item> {
	/// Every item in the batch succeeded.
	pub fn all_cleared(items: Vec<Item>) -> Self {
		Self {
			cleared: items,
			failed: Vec::new(),
		}
	}

	/// The whole batch failed with a single error (the all-or-nothing case, e.g.
	/// one S3 multi-delete request).
	pub fn whole_batch_failed(
		items: Vec<Item>,
		error: anyhow::Error,
	) -> Self {
		Self {
			cleared: Vec::new(),
			failed: vec![FailedGroup {
				items,
				error,
			}],
		}
	}
}

/// A transactional outbox table: a set of claimable job rows under a lease/retry
/// policy. `claim` leases a batch (bumping attempts and the next-attempt time),
/// `clear` removes finished rows, and `mark_failed` records an error and
/// reschedules.
///
/// The methods return futures with an explicit `Send` bound because
/// `drain_outbox` is awaited inside a `tokio::spawn`ed worker task (the same
/// reason as `worker::MaintenanceTask`); a bare async-fn-in-trait could not
/// express that bound at the generic call site.
pub trait OutboxQueue {
	type Item: Send;

	fn claim(
		&mut self,
		retry: &OutboxRetryConfig,
	) -> impl Future<Output = Result<Vec<Self::Item>, AppError>> + Send;

	fn clear(
		&mut self,
		items: &[Self::Item],
	) -> impl Future<Output = Result<(), AppError>> + Send;

	fn mark_failed(
		&mut self,
		items: &[Self::Item],
		error_message: &str,
		retry_after_seconds: i64,
	) -> impl Future<Output = Result<(), AppError>> + Send;
}

/// The side-effecting work for one claimed batch (delete objects from storage,
/// send emails). Returning a [`DrainOutcome`] lets the all-or-nothing shape
/// (storage: one request for the whole batch) and the per-item shape (email: one
/// send per message) both be expressed without `drain_outbox` knowing which is
/// which.
pub trait OutboxProcessor {
	type Item;

	fn process(
		&self,
		items: Vec<Self::Item>,
	) -> impl Future<Output = DrainOutcome<Self::Item>> + Send;
}

/// Drains an outbox to completion.
///
/// Claims a batch, processes it, clears the successes, and marks each failed
/// group for retry. A failure does not abort the drain: the loop keeps claiming
/// (rows just marked failed are rescheduled past `now()`, so they are not
/// re-claimed this pass), and the first error is surfaced once the queue empties
/// so the worker still records that something went wrong. Bounded by the queue's
/// `max_attempts`: a permanently broken row stops being claimable, so the loop
/// cannot spin forever.
pub async fn drain_outbox<Q, P>(
	queue: &mut Q,
	processor: &P,
	retry: &OutboxRetryConfig,
) -> Result<(), AppError>
where
	Q: OutboxQueue,
	P: OutboxProcessor<Item = Q::Item>, {
	let mut first_error: Option<AppError> = None;
	loop {
		let items = queue.claim(retry).await?;
		if items.is_empty() {
			break;
		}

		let DrainOutcome {
			cleared,
			failed,
		} = processor.process(items).await;
		if !cleared.is_empty() {
			queue.clear(&cleared).await?;
		}
		for group in failed {
			let error_message = group.error.to_string();
			queue.mark_failed(&group.items, &error_message, retry.retry_seconds).await?;
			if first_error.is_none() {
				first_error = Some(AppError::Internal(group.error));
			}
		}
	}

	match first_error {
		Some(error) => Err(error),
		None => Ok(()),
	}
}

#[cfg(test)]
mod tests {
	use {
		super::{
			DrainOutcome,
			OutboxProcessor,
			OutboxQueue,
			OutboxRetryConfig,
			drain_outbox,
		},
		crate::errors::AppError,
		std::collections::VecDeque,
	};

	fn retry() -> OutboxRetryConfig {
		OutboxRetryConfig {
			retry_seconds: 17,
			lease_seconds: 300,
			batch_size: 1000,
			max_attempts: 10,
		}
	}

	#[derive(Default)]
	struct FakeQueue {
		pending: VecDeque<Vec<String>>,
		claims: Vec<OutboxRetryConfig>,
		cleared: Vec<Vec<String>>,
		failed: Vec<(Vec<String>, String, i64)>,
	}

	impl OutboxQueue for FakeQueue {
		type Item = String;

		async fn claim(
			&mut self,
			retry: &OutboxRetryConfig,
		) -> Result<Vec<String>, AppError> {
			self.claims.push(*retry);
			Ok(self.pending.pop_front().unwrap_or_default())
		}

		async fn clear(
			&mut self,
			items: &[String],
		) -> Result<(), AppError> {
			self.cleared.push(items.to_vec());
			Ok(())
		}

		async fn mark_failed(
			&mut self,
			items: &[String],
			error_message: &str,
			retry_after_seconds: i64,
		) -> Result<(), AppError> {
			self.failed.push((items.to_vec(), error_message.to_string(), retry_after_seconds));
			Ok(())
		}
	}

	/// Processor that succeeds on the whole batch.
	struct ClearAll;

	impl OutboxProcessor for ClearAll {
		type Item = String;

		async fn process(
			&self,
			items: Vec<String>,
		) -> DrainOutcome<String> {
			DrainOutcome::all_cleared(items)
		}
	}

	/// Processor that fails the whole batch with a fixed message.
	struct FailAll(&'static str);

	impl OutboxProcessor for FailAll {
		type Item = String;

		async fn process(
			&self,
			items: Vec<String>,
		) -> DrainOutcome<String> {
			DrainOutcome::whole_batch_failed(items, anyhow::anyhow!(self.0))
		}
	}

	#[tokio::test]
	async fn drain_outbox_clears_all_pending_batches() -> anyhow::Result<()> {
		let mut queue = FakeQueue {
			pending: vec![vec!["a".to_string(), "b".to_string()], vec!["c".to_string()]].into(),
			..FakeQueue::default()
		};

		drain_outbox(&mut queue, &ClearAll, &retry()).await?;

		assert_eq!(
			queue.cleared,
			vec![vec!["a".to_string(), "b".to_string()], vec!["c".to_string()]]
		);
		assert!(queue.failed.is_empty());
		// Two batches plus the empty claim that ends the loop, all with the same policy.
		assert_eq!(queue.claims, vec![retry(), retry(), retry()]);
		Ok(())
	}

	#[tokio::test]
	async fn drain_outbox_marks_failures_and_surfaces_first_error() -> anyhow::Result<()> {
		let mut queue = FakeQueue {
			pending: vec![vec!["x".to_string(), "y".to_string()]].into(),
			..FakeQueue::default()
		};

		let Err(error) = drain_outbox(&mut queue, &FailAll("boom"), &retry()).await else {
			anyhow::bail!("a processing failure should surface as an error");
		};

		assert!(matches!(error, AppError::Internal(_)));
		assert!(queue.cleared.is_empty());
		assert_eq!(
			queue.failed,
			vec![(vec!["x".to_string(), "y".to_string()], "boom".to_string(), 17)]
		);
		// One claim returns the failing batch, a second returns empty and ends the
		// drain: failures are rescheduled, not retried in the same pass.
		assert_eq!(queue.claims.len(), 2);
		Ok(())
	}
}
