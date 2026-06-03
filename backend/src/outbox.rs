//! Shared primitives for the transactional outbox / lease-retry pattern used by
//! the storage-deletion and email-outbox workers.

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
