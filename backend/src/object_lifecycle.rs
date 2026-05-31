use {
	crate::{
		db::queries::{
			DELETE_OBJECT_ALLOWED_USERS_QUERY,
			DELETE_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECTS_QUERY,
			INSERT_OBJECT_ALLOWED_USER_QUERY,
			INSERT_OBJECT_QUERY,
			INSERT_OBJECT_STORAGE_DELETIONS_QUERY,
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
			SELECT_PENDING_OBJECT_STORAGE_DELETIONS_QUERY,
			SELECT_USERS_BY_EMAILS_QUERY,
			UPDATE_OBJECT_QUERY,
			UPSERT_OBJECT_QUERY,
		},
		errors::AppError,
		graphql::objects::{
			location::Location,
			s3_object::{
				PublicityOverride,
				S3Object,
			},
		},
		storage::StorageClient,
	},
	anyhow::Context,
	aws_sdk_s3::primitives::ByteStream,
	deadpool_postgres::Client,
	futures::future::join_all,
	jiff::Timestamp,
	tokio_postgres::{
		Row,
		Transaction,
		error::SqlState,
	},
};

const STORAGE_DELETION_OUTBOX_BATCH_SIZE: i64 = 1000;

pub struct ObjectLifecycleService<'a> {
	db_client: &'a mut Client,
	storage: &'a StorageClient,
}

pub struct ObjectUpload {
	pub name: String,
	pub bytes: ByteStream,
	pub content_type: String,
	pub made_on: Option<String>,
	pub location: Option<Location>,
	pub user_id: i64,
	pub publicity: PublicityOverride,
	pub allowed_users: Vec<String>,
}

impl<'a> ObjectLifecycleService<'a> {
	pub fn new(
		db_client: &'a mut Client,
		storage: &'a StorageClient,
	) -> Self {
		Self {
			db_client,
			storage,
		}
	}

	pub async fn upload_and_create_object(
		&mut self,
		upload: ObjectUpload,
	) -> Result<S3Object, AppError> {
		let parsed_made_on = parse_made_on(upload.made_on)?;
		let location_geometry = location_geometry(upload.location.as_ref())?;
		let transaction = self.db_client.transaction().await?;
		let mut s3_object = S3Object::try_from(
			transaction
				.query_one(
					INSERT_OBJECT_QUERY,
					&[
						&upload.name,
						&parsed_made_on,
						&location_geometry,
						&upload.user_id,
						&upload.publicity,
					],
				)
				.await
				.map_err(|error| insert_object_error(error, &upload.name))?,
		)
		.await
		.map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message)
		})?;
		let id = object_id(&s3_object)?;
		s3_object.allowed_users =
			replace_allowed_users(&transaction, id, upload.allowed_users).await?;

		self.storage.upload_object(&upload.name, upload.bytes, upload.content_type).await?;

		match transaction.commit().await {
			Ok(()) => Ok(s3_object),
			Err(error) => {
				if let Err(cleanup_error) =
					self.storage.delete_objects(std::slice::from_ref(&upload.name)).await
				{
					tracing::error!(
						object_name = %upload.name,
						error = ?cleanup_error,
						"Failed to roll back uploaded object after database persistence failed"
					);
				}
				Err(AppError::from(error))
			}
		}
	}

	pub async fn delete_objects(
		&mut self,
		ids: &[i64],
	) -> Result<Vec<S3Object>, AppError> {
		tracing::debug!("IDs to delete: {:?}", ids);
		let transaction = self.db_client.transaction().await?;
		let rows = transaction.query(DELETE_OBJECTS_QUERY, &[&ids]).await?;
		let objects = collect_s3_objects(rows).await?;
		let object_names = objects.iter().map(|object| object.name.clone()).collect::<Vec<_>>();

		if !object_names.is_empty() {
			transaction
				.execute(INSERT_OBJECT_STORAGE_DELETIONS_QUERY, &[&object_names])
				.await
				.context("Failed to enqueue object storage deletions")?;
		}

		transaction.commit().await?;

		if let Err(error) = self.drain_storage_deletions().await {
			tracing::error!(
				error = ?error,
				"Failed to drain object storage deletions after database delete"
			);
		}

		Ok(objects)
	}

	pub async fn drain_storage_deletions(&mut self) -> Result<(), AppError> {
		drain_storage_deletion_outbox(self.db_client, self.storage).await
	}
}

trait StorageDeletionSink {
	async fn delete_objects(
		&self,
		object_names: &[String],
	) -> anyhow::Result<()>;
}

impl StorageDeletionSink for StorageClient {
	async fn delete_objects(
		&self,
		object_names: &[String],
	) -> anyhow::Result<()> {
		StorageClient::delete_objects(self, object_names).await
	}
}

trait StorageDeletionOutbox {
	async fn pending_storage_deletions(
		&mut self,
		limit: i64,
	) -> Result<Vec<String>, AppError>;

	async fn clear_storage_deletions(
		&mut self,
		object_names: &[String],
	) -> Result<(), AppError>;

	async fn mark_storage_deletions_failed(
		&mut self,
		object_names: &[String],
		error_message: &str,
	) -> Result<(), AppError>;
}

impl StorageDeletionOutbox for Client {
	async fn pending_storage_deletions(
		&mut self,
		limit: i64,
	) -> Result<Vec<String>, AppError> {
		let rows = self
			.query(SELECT_PENDING_OBJECT_STORAGE_DELETIONS_QUERY, &[&limit])
			.await
			.context("Failed to load pending object storage deletions")?;
		rows.into_iter()
			.map(|row| row.try_get("object_name"))
			.collect::<Result<Vec<String>, _>>()
			.context("Failed to read pending object storage deletion row")
			.map_err(AppError::from)
	}

	async fn clear_storage_deletions(
		&mut self,
		object_names: &[String],
	) -> Result<(), AppError> {
		self.execute(DELETE_OBJECT_STORAGE_DELETIONS_QUERY, &[&object_names])
			.await
			.context("Failed to clear completed object storage deletions")?;
		Ok(())
	}

	async fn mark_storage_deletions_failed(
		&mut self,
		object_names: &[String],
		error_message: &str,
	) -> Result<(), AppError> {
		self.execute(MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY, &[&object_names, &error_message])
			.await
			.context("Failed to record object storage deletion failure")?;
		Ok(())
	}
}

async fn drain_storage_deletion_outbox(
	outbox: &mut impl StorageDeletionOutbox,
	storage: &impl StorageDeletionSink,
) -> Result<(), AppError> {
	loop {
		let object_names =
			outbox.pending_storage_deletions(STORAGE_DELETION_OUTBOX_BATCH_SIZE).await?;

		if object_names.is_empty() {
			return Ok(());
		}

		if let Err(error) = storage.delete_objects(&object_names).await {
			let error_message = error.to_string();
			outbox.mark_storage_deletions_failed(&object_names, &error_message).await?;
			return Err(AppError::Internal(error));
		}

		outbox.clear_storage_deletions(&object_names).await?;
	}
}

pub async fn update_s3_object_metadata(
	client: &mut Client,
	id: i64,
	name: String,
	made_on: Option<String>,
	location: Option<Location>,
	publicity: PublicityOverride,
	allowed_users: Vec<String>,
) -> Result<S3Object, AppError> {
	let parsed_made_on = parse_made_on(made_on)?;
	let location_geometry = location_geometry(location.as_ref())?;
	if let Some(location_geometry) = &location_geometry {
		tracing::debug!("Formatted location geometry: {}", location_geometry);
	}
	tracing::debug!(
		"Executing update with: id={}, name={}, made_on={:?}, location={:?}, publicity={:?}",
		id,
		name,
		parsed_made_on.as_ref().map(|ts| ts.to_string()),
		location_geometry,
		publicity
	);

	let transaction = client.transaction().await?;
	let mut s3_object = S3Object::try_from(
		transaction
			.query_one(
				UPDATE_OBJECT_QUERY,
				&[&id, &name, &parsed_made_on, &location_geometry, &publicity],
			)
			.await?,
	)
	.await
	.map_err(|e| anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message))?;

	s3_object.allowed_users = replace_allowed_users(&transaction, id, allowed_users).await?;
	transaction.commit().await?;
	Ok(s3_object)
}

pub async fn upsert_s3_object_metadata(
	client: &mut Client,
	name: String,
	made_on: Option<String>,
	location: Option<Location>,
	user_id: i64,
	publicity: PublicityOverride,
	allowed_users: Vec<String>,
) -> Result<S3Object, AppError> {
	let parsed_made_on = parse_made_on(made_on)?;
	let location_geometry = location_geometry(location.as_ref())?;
	if let Some(location_geometry) = &location_geometry {
		tracing::debug!("Formatted location geometry: {}", location_geometry);
	}
	tracing::debug!(
		"Executing upsert with: name={}, made_on={:?}, location={:?}, user_id={}, publicity={:?}",
		name,
		parsed_made_on.as_ref().map(|ts| ts.to_string()),
		location_geometry,
		user_id,
		publicity
	);

	let transaction = client.transaction().await?;
	let mut s3_object = S3Object::try_from(
		transaction
			.query_one(
				UPSERT_OBJECT_QUERY,
				&[&name, &parsed_made_on, &location_geometry, &user_id, &publicity],
			)
			.await?,
	)
	.await
	.map_err(|e| anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message))?;
	let id = object_id(&s3_object)?;

	s3_object.allowed_users = replace_allowed_users(&transaction, id, allowed_users).await?;
	transaction.commit().await?;
	Ok(s3_object)
}

fn parse_made_on(made_on: Option<String>) -> Result<Option<Timestamp>, AppError> {
	match made_on {
		Some(timestamp_string) => Ok(Some(
			timestamp_string
				.parse()
				.map_err(|e| AppError::Validation(format!("Invalid timestamp format: {e}")))?,
		)),
		None => Ok(None),
	}
}

fn location_geometry(location: Option<&Location>) -> Result<Option<String>, AppError> {
	location.map(Location::geometry).transpose()
}

async fn collect_s3_objects(rows: Vec<Row>) -> Result<Vec<S3Object>, AppError> {
	join_all(rows.into_iter().map(S3Object::try_from))
		.await
		.into_iter()
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| {
			AppError::Internal(anyhow::anyhow!(
				"Failed to convert database rows to S3 objects: {}",
				e.message
			))
		})
}

async fn replace_allowed_users(
	transaction: &Transaction<'_>,
	object_id: i64,
	allowed_users: Vec<String>,
) -> Result<Vec<String>, AppError> {
	transaction
		.execute(DELETE_OBJECT_ALLOWED_USERS_QUERY, &[&object_id])
		.await
		.context("Failed to delete object allowed users from database")?;

	let mut valid_allowed_users = Vec::new();

	if !allowed_users.is_empty() {
		let rows = transaction.query(SELECT_USERS_BY_EMAILS_QUERY, &[&allowed_users]).await?;

		for row in rows {
			let user_id: i64 =
				row.try_get("id").context("Failed to get user ID from database row")?;
			let email: String =
				row.try_get("email").context("Failed to get email from database row")?;
			transaction
				.execute(INSERT_OBJECT_ALLOWED_USER_QUERY, &[&object_id, &user_id])
				.await
				.context("Failed to insert object allowed user into database")?;
			valid_allowed_users.push(email);
		}
	}

	Ok(valid_allowed_users)
}

fn object_id(object: &S3Object) -> Result<i64, AppError> {
	object
		.id
		.parse()
		.map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to parse S3 object ID: {e}")))
}

fn insert_object_error(
	error: tokio_postgres::Error,
	name: &str,
) -> AppError {
	if error.as_db_error().is_some_and(|db_error| db_error.code() == &SqlState::UNIQUE_VIOLATION) {
		return AppError::Validation(format!("Object named '{name}' already exists"));
	}

	AppError::from(error)
}

#[cfg(test)]
mod tests {
	use {
		super::{
			AppError,
			STORAGE_DELETION_OUTBOX_BATCH_SIZE,
			StorageDeletionOutbox,
			StorageDeletionSink,
			drain_storage_deletion_outbox,
		},
		std::{
			collections::VecDeque,
			sync::Mutex,
		},
	};

	#[derive(Default)]
	struct FakeStorageDeletionOutbox {
		pending_batches: VecDeque<Vec<String>>,
		requested_limits: Vec<i64>,
		cleared_batches: Vec<Vec<String>>,
		failed_batches: Vec<(Vec<String>, String)>,
	}

	impl FakeStorageDeletionOutbox {
		fn with_pending_batches(pending_batches: Vec<Vec<String>>) -> Self {
			Self {
				pending_batches: pending_batches.into(),
				..Default::default()
			}
		}
	}

	impl StorageDeletionOutbox for FakeStorageDeletionOutbox {
		async fn pending_storage_deletions(
			&mut self,
			limit: i64,
		) -> Result<Vec<String>, AppError> {
			self.requested_limits.push(limit);
			Ok(self.pending_batches.pop_front().unwrap_or_default())
		}

		async fn clear_storage_deletions(
			&mut self,
			object_names: &[String],
		) -> Result<(), AppError> {
			self.cleared_batches.push(object_names.to_vec());
			Ok(())
		}

		async fn mark_storage_deletions_failed(
			&mut self,
			object_names: &[String],
			error_message: &str,
		) -> Result<(), AppError> {
			self.failed_batches.push((object_names.to_vec(), error_message.to_string()));
			Ok(())
		}
	}

	#[derive(Default)]
	struct FakeStorageDeletionSink {
		deleted_batches: Mutex<Vec<Vec<String>>>,
		error_message: Option<&'static str>,
	}

	impl FakeStorageDeletionSink {
		fn failing(error_message: &'static str) -> Self {
			Self {
				deleted_batches: Mutex::new(Vec::new()),
				error_message: Some(error_message),
			}
		}

		fn deleted_batches(&self) -> anyhow::Result<Vec<Vec<String>>> {
			let deleted_batches = self
				.deleted_batches
				.lock()
				.map_err(|_| anyhow::anyhow!("deleted batch lock is poisoned"))?;
			Ok(deleted_batches.clone())
		}
	}

	impl StorageDeletionSink for FakeStorageDeletionSink {
		async fn delete_objects(
			&self,
			object_names: &[String],
		) -> anyhow::Result<()> {
			if let Some(error_message) = self.error_message {
				anyhow::bail!(error_message);
			}

			let mut deleted_batches = self
				.deleted_batches
				.lock()
				.map_err(|_| anyhow::anyhow!("deleted batch lock is poisoned"))?;
			deleted_batches.push(object_names.to_vec());
			Ok(())
		}
	}

	#[tokio::test]
	async fn drain_storage_deletion_outbox_clears_all_pending_batches() -> anyhow::Result<()> {
		let first_batch = object_names("first", STORAGE_DELETION_OUTBOX_BATCH_SIZE as usize);
		let second_batch = object_names("second", 2);
		let mut outbox = FakeStorageDeletionOutbox::with_pending_batches(vec![
			first_batch.clone(),
			second_batch.clone(),
		]);
		let storage = FakeStorageDeletionSink::default();

		drain_storage_deletion_outbox(&mut outbox, &storage).await?;

		assert_eq!(
			outbox.requested_limits,
			vec![
				STORAGE_DELETION_OUTBOX_BATCH_SIZE,
				STORAGE_DELETION_OUTBOX_BATCH_SIZE,
				STORAGE_DELETION_OUTBOX_BATCH_SIZE,
			]
		);
		assert_eq!(storage.deleted_batches()?, vec![first_batch.clone(), second_batch.clone()]);
		assert_eq!(outbox.cleared_batches, vec![first_batch, second_batch]);
		assert!(outbox.failed_batches.is_empty());

		Ok(())
	}

	#[tokio::test]
	async fn drain_storage_deletion_outbox_records_storage_delete_failure() -> anyhow::Result<()> {
		let pending_batch = object_names("failed", 2);
		let mut outbox =
			FakeStorageDeletionOutbox::with_pending_batches(vec![pending_batch.clone()]);
		let storage = FakeStorageDeletionSink::failing("storage delete failed");

		let Err(error) = drain_storage_deletion_outbox(&mut outbox, &storage).await else {
			anyhow::bail!("storage delete failure should stop the drain");
		};

		assert!(matches!(error, AppError::Internal(_)));
		assert!(storage.deleted_batches()?.is_empty());
		assert!(outbox.cleared_batches.is_empty());
		assert_eq!(
			outbox.failed_batches,
			vec![(pending_batch, "storage delete failed".to_string())]
		);
		assert_eq!(outbox.requested_limits, vec![STORAGE_DELETION_OUTBOX_BATCH_SIZE]);

		Ok(())
	}

	fn object_names(
		prefix: &str,
		count: usize,
	) -> Vec<String> {
		(0 .. count).map(|index| format!("{prefix}-{index}")).collect()
	}
}
