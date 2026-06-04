use {
	crate::{
		db::queries::{
			CLAIM_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECTS_BY_STORAGE_KEYS_QUERY,
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
		},
		errors::AppError,
		outbox::{
			DrainOutcome,
			OutboxProcessor,
			OutboxQueue,
			OutboxRetryConfig,
		},
		storage::StorageClient,
	},
	anyhow::Context,
	deadpool_postgres::Client,
};

/// The object-storage-deletion outbox as an [`OutboxQueue`]. `clear` is a
/// two-statement transaction because a finished deletion removes both the queue
/// row and the object-metadata row it points at.
pub(super) struct StorageDeletionQueue<'a>(pub(super) &'a mut Client);

impl OutboxQueue for StorageDeletionQueue<'_> {
	type Item = String;

	async fn claim(
		&mut self,
		retry: &OutboxRetryConfig,
	) -> Result<Vec<String>, AppError> {
		let rows = self
			.0
			.query(
				CLAIM_OBJECT_STORAGE_DELETIONS_QUERY,
				&[&retry.batch_size, &retry.lease_seconds, &retry.max_attempts],
			)
			.await
			.context("Failed to claim pending object storage deletions")?;
		rows.into_iter()
			.map(|row| row.try_get("storage_key"))
			.collect::<Result<Vec<_>, _>>()
			.context("Failed to read claimed object storage deletion row")
			.map_err(AppError::from)
	}

	async fn clear(
		&mut self,
		storage_keys: &[String],
	) -> Result<(), AppError> {
		let transaction = self.0.transaction().await?;
		transaction
			.execute(DELETE_OBJECTS_BY_STORAGE_KEYS_QUERY, &[&storage_keys])
			.await
			.context("Failed to delete completed object metadata rows")?;
		transaction
			.execute(DELETE_OBJECT_STORAGE_DELETIONS_QUERY, &[&storage_keys])
			.await
			.context("Failed to clear completed object storage deletions")?;
		transaction.commit().await?;
		Ok(())
	}

	async fn mark_failed(
		&mut self,
		storage_keys: &[String],
		error_message: &str,
		retry_after_seconds: i64,
	) -> Result<(), AppError> {
		self.0
			.execute(
				MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
				&[&storage_keys, &error_message, &retry_after_seconds],
			)
			.await
			.context("Failed to record object storage deletion failure")?;
		Ok(())
	}
}

/// Deletes a claimed batch of storage keys in one S3 multi-delete request. The
/// request is all-or-nothing, so the whole batch clears or the whole batch is
/// marked failed.
pub(super) struct StorageDeletionProcessor<'a> {
	pub(super) storage: &'a StorageClient,
}

impl OutboxProcessor for StorageDeletionProcessor<'_> {
	type Item = String;

	async fn process(
		&self,
		storage_keys: Vec<String>,
	) -> DrainOutcome<String> {
		match self.storage.delete_objects(&storage_keys).await {
			Ok(()) => DrainOutcome::all_cleared(storage_keys),
			Err(error) => DrainOutcome::whole_batch_failed(storage_keys, error),
		}
	}
}
