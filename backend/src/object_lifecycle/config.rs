use {
	crate::outbox::{
		OutboxRetryConfig,
		ensure_positive,
	},
	anyhow::Context,
	serde::Deserialize,
	std::time::Duration,
};

#[derive(Clone, Debug, Deserialize)]
pub struct ObjectLifecycleConfig {
	#[serde(default = "ObjectLifecycleConfig::default_pending_upload_timeout_seconds")]
	pub pending_upload_timeout_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_upload_max_file_size_bytes")]
	pub upload_max_file_size_bytes: i64,
	#[serde(default = "ObjectLifecycleConfig::default_upload_part_size_bytes")]
	pub upload_part_size_bytes: i64,
	#[serde(default = "ObjectLifecycleConfig::default_upload_max_part_count")]
	pub upload_max_part_count: i32,
	#[serde(default = "ObjectLifecycleConfig::default_upload_session_ttl_seconds")]
	pub upload_session_ttl_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_upload_session_cleanup_retry_seconds")]
	pub upload_session_cleanup_retry_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_upload_session_cleanup_lease_seconds")]
	pub upload_session_cleanup_lease_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_upload_session_cleanup_max_attempts")]
	pub upload_session_cleanup_max_attempts: i32,
	#[serde(default = "ObjectLifecycleConfig::default_upload_session_cleanup_batch_size")]
	pub upload_session_cleanup_batch_size: i64,
	#[serde(default = "ObjectLifecycleConfig::default_storage_deletion_retry_seconds")]
	pub storage_deletion_retry_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_storage_deletion_lease_seconds")]
	pub storage_deletion_lease_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_maintenance_interval_seconds")]
	pub maintenance_interval_seconds: i64,
	#[serde(default = "ObjectLifecycleConfig::default_storage_deletion_batch_size")]
	pub storage_deletion_batch_size: i64,
	#[serde(default = "ObjectLifecycleConfig::default_storage_deletion_max_attempts")]
	pub storage_deletion_max_attempts: i32,
}

impl ObjectLifecycleConfig {
	pub const S3_MAX_MULTIPART_PART_COUNT: i32 = 10_000;
	pub const S3_MAX_OBJECT_SIZE_BYTES: i64 = 5 * 1024 * 1024 * 1024 * 1024;
	pub const S3_MIN_MULTIPART_PART_SIZE_BYTES: i64 = 5 * 1024 * 1024;

	pub const fn default_pending_upload_timeout_seconds() -> i64 {
		3600
	}

	pub const fn default_upload_max_file_size_bytes() -> i64 {
		1024 * 1024 * 1024
	}

	pub const fn default_upload_part_size_bytes() -> i64 {
		8 * 1024 * 1024
	}

	pub const fn default_upload_max_part_count() -> i32 {
		Self::S3_MAX_MULTIPART_PART_COUNT
	}

	pub const fn default_upload_session_ttl_seconds() -> i64 {
		3600
	}

	pub const fn default_upload_session_cleanup_retry_seconds() -> i64 {
		60
	}

	pub const fn default_upload_session_cleanup_lease_seconds() -> i64 {
		300
	}

	pub const fn default_upload_session_cleanup_max_attempts() -> i32 {
		10
	}

	pub const fn default_upload_session_cleanup_batch_size() -> i64 {
		1000
	}

	pub const fn default_storage_deletion_retry_seconds() -> i64 {
		60
	}

	pub const fn default_storage_deletion_lease_seconds() -> i64 {
		300
	}

	pub const fn default_maintenance_interval_seconds() -> i64 {
		30
	}

	/// Default claim size. The storage layer owns S3's per-request multi-delete limit
	/// and chunks internally; this can be any positive value, and 1000 happens to match
	/// S3's cap so a typical claim fires exactly one storage request.
	pub const fn default_storage_deletion_batch_size() -> i64 {
		1000
	}

	pub const fn default_storage_deletion_max_attempts() -> i32 {
		10
	}

	pub fn validate(&self) -> anyhow::Result<()> {
		ensure_positive!(self, pending_upload_timeout_seconds);
		if self.upload_max_file_size_bytes <= 0 {
			anyhow::bail!("upload_max_file_size_bytes must be greater than 0");
		}
		if self.upload_max_file_size_bytes > Self::S3_MAX_OBJECT_SIZE_BYTES {
			anyhow::bail!(
				"upload_max_file_size_bytes must be at most {}",
				Self::S3_MAX_OBJECT_SIZE_BYTES
			);
		}
		if self.upload_part_size_bytes < Self::S3_MIN_MULTIPART_PART_SIZE_BYTES {
			anyhow::bail!(
				"upload_part_size_bytes must be at least {}",
				Self::S3_MIN_MULTIPART_PART_SIZE_BYTES
			);
		}
		ensure_positive!(self, upload_max_part_count);
		if self.upload_max_part_count > Self::S3_MAX_MULTIPART_PART_COUNT {
			anyhow::bail!(
				"upload_max_part_count must be at most {}",
				Self::S3_MAX_MULTIPART_PART_COUNT
			);
		}
		ensure_positive!(self, upload_session_ttl_seconds);
		self.upload_session_cleanup().validate("upload_session_cleanup")?;
		self.upload_session_total_parts(self.upload_max_file_size_bytes)?;
		ensure_positive!(self, maintenance_interval_seconds);
		self.storage_deletion().validate("storage_deletion")?;
		Ok(())
	}

	/// Lease/retry policy for the storage-deletion outbox, as a runtime view.
	pub fn storage_deletion(&self) -> OutboxRetryConfig {
		OutboxRetryConfig {
			retry_seconds: self.storage_deletion_retry_seconds,
			lease_seconds: self.storage_deletion_lease_seconds,
			batch_size: self.storage_deletion_batch_size,
			max_attempts: self.storage_deletion_max_attempts,
		}
	}

	/// Lease/retry policy for the expired-upload-session cleanup, as a runtime view.
	pub fn upload_session_cleanup(&self) -> OutboxRetryConfig {
		OutboxRetryConfig {
			retry_seconds: self.upload_session_cleanup_retry_seconds,
			lease_seconds: self.upload_session_cleanup_lease_seconds,
			batch_size: self.upload_session_cleanup_batch_size,
			max_attempts: self.upload_session_cleanup_max_attempts,
		}
	}

	pub fn upload_session_total_parts(
		&self,
		file_size_bytes: i64,
	) -> anyhow::Result<i32> {
		if file_size_bytes <= 0 {
			anyhow::bail!("file_size_bytes must be greater than 0");
		}
		if file_size_bytes > self.upload_max_file_size_bytes {
			anyhow::bail!("file_size_bytes must be at most {}", self.upload_max_file_size_bytes);
		}
		let count = part_count(file_size_bytes, self.upload_part_size_bytes);
		if count > i64::from(self.upload_max_part_count) {
			anyhow::bail!(
				"file_size_bytes requires {count} parts, exceeding upload_max_part_count {}",
				self.upload_max_part_count
			);
		}
		i32::try_from(count).context("upload part count exceeds i32 range")
	}

	pub fn upload_session_expected_part_size_bytes(
		&self,
		file_size_bytes: i64,
		part_number: i32,
	) -> anyhow::Result<i64> {
		let total_parts = self.upload_session_total_parts(file_size_bytes)?;
		if !(1 ..= total_parts).contains(&part_number) {
			anyhow::bail!("part_number must be between 1 and {total_parts}");
		}
		Ok(part_length(file_size_bytes, self.upload_part_size_bytes, part_number))
	}

	/// Returns Self if `validate` succeeds; convenient for builder-style construction.
	pub fn validated(self) -> anyhow::Result<Self> {
		self.validate()?;
		Ok(self)
	}

	pub(super) fn worker_interval(&self) -> Duration {
		Duration::from_secs(self.maintenance_interval_seconds as u64)
	}
}

impl Default for ObjectLifecycleConfig {
	fn default() -> Self {
		Self {
			pending_upload_timeout_seconds: Self::default_pending_upload_timeout_seconds(),
			upload_max_file_size_bytes: Self::default_upload_max_file_size_bytes(),
			upload_part_size_bytes: Self::default_upload_part_size_bytes(),
			upload_max_part_count: Self::default_upload_max_part_count(),
			upload_session_ttl_seconds: Self::default_upload_session_ttl_seconds(),
			upload_session_cleanup_retry_seconds:
				Self::default_upload_session_cleanup_retry_seconds(),
			upload_session_cleanup_lease_seconds:
				Self::default_upload_session_cleanup_lease_seconds(),
			upload_session_cleanup_max_attempts: Self::default_upload_session_cleanup_max_attempts(
			),
			upload_session_cleanup_batch_size: Self::default_upload_session_cleanup_batch_size(),
			storage_deletion_retry_seconds: Self::default_storage_deletion_retry_seconds(),
			storage_deletion_lease_seconds: Self::default_storage_deletion_lease_seconds(),
			maintenance_interval_seconds: Self::default_maintenance_interval_seconds(),
			storage_deletion_batch_size: Self::default_storage_deletion_batch_size(),
			storage_deletion_max_attempts: Self::default_storage_deletion_max_attempts(),
		}
	}
}

/// Number of equal parts (the last possibly shorter) needed to cover
/// `file_size_bytes` at `part_size_bytes`. Both must be positive.
///
/// The two callers layer their own part-count cap on top, and the caps differ
/// intentionally: the config enforces the operator's `upload_max_part_count` at
/// session creation, while the session-based path enforces the immutable S3
/// limit at completion (the session was already validated against the config).
pub(super) fn part_count(
	file_size_bytes: i64,
	part_size_bytes: i64,
) -> i64 {
	((file_size_bytes - 1) / part_size_bytes) + 1
}

/// Byte length of the 1-based `part_number` for a `file_size_bytes` upload split
/// at `part_size_bytes` (the final part is whatever remains). `part_number` must
/// be in range.
pub(super) fn part_length(
	file_size_bytes: i64,
	part_size_bytes: i64,
	part_number: i32,
) -> i64 {
	let part_start = i64::from(part_number - 1) * part_size_bytes;
	part_size_bytes.min(file_size_bytes - part_start)
}

#[cfg(test)]
mod tests {
	use super::ObjectLifecycleConfig;

	#[test]
	fn object_lifecycle_config_rejects_invalid_values() {
		let valid = ObjectLifecycleConfig::default();
		assert!(valid.clone().validated().is_ok());

		// Each second-valued field must be > 0; zero or negative fails validate().
		for invalid in [
			ObjectLifecycleConfig {
				pending_upload_timeout_seconds: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_max_file_size_bytes: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_max_file_size_bytes: ObjectLifecycleConfig::S3_MAX_OBJECT_SIZE_BYTES + 1,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_part_size_bytes: ObjectLifecycleConfig::S3_MIN_MULTIPART_PART_SIZE_BYTES - 1,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_max_part_count: ObjectLifecycleConfig::S3_MAX_MULTIPART_PART_COUNT + 1,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_session_ttl_seconds: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_session_cleanup_retry_seconds: -1,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_session_cleanup_lease_seconds: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_session_cleanup_max_attempts: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_session_cleanup_batch_size: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				upload_max_file_size_bytes: (i64::from(valid.upload_max_part_count) *
					valid.upload_part_size_bytes) +
					1,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				storage_deletion_retry_seconds: -1,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				storage_deletion_lease_seconds: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				maintenance_interval_seconds: -5,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				storage_deletion_batch_size: 0,
				..valid.clone()
			},
			ObjectLifecycleConfig {
				storage_deletion_max_attempts: 0,
				..valid.clone()
			},
		] {
			assert!(invalid.validate().is_err());
		}

		// A very large positive value is valid.
		let extreme = ObjectLifecycleConfig {
			pending_upload_timeout_seconds: i64::MAX,
			..valid
		};
		assert!(extreme.validate().is_ok());
	}

	#[test]
	fn upload_session_policy_calculates_part_counts_and_lengths() -> anyhow::Result<()> {
		let part_size = ObjectLifecycleConfig::S3_MIN_MULTIPART_PART_SIZE_BYTES;
		let config = ObjectLifecycleConfig {
			upload_max_file_size_bytes: part_size * 4,
			upload_part_size_bytes: part_size,
			upload_max_part_count: 4,
			..ObjectLifecycleConfig::default()
		};
		let file_size = (part_size * 2) + 7;

		assert_eq!(config.upload_session_total_parts(1)?, 1);
		assert_eq!(config.upload_session_total_parts(part_size)?, 1);
		assert_eq!(config.upload_session_total_parts(file_size)?, 3);
		assert_eq!(config.upload_session_expected_part_size_bytes(file_size, 1)?, part_size);
		assert_eq!(config.upload_session_expected_part_size_bytes(file_size, 2)?, part_size);
		assert_eq!(config.upload_session_expected_part_size_bytes(file_size, 3)?, 7);
		assert!(config.upload_session_total_parts(0).is_err());
		assert!(config.upload_session_expected_part_size_bytes(file_size, 4).is_err());

		Ok(())
	}
}
