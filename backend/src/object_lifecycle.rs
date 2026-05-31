use {
	crate::{
		db::queries::{
			CLAIM_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECT_ALLOWED_USERS_QUERY,
			DELETE_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECTS_BY_STORAGE_KEYS_QUERY,
			FINALIZE_OBJECT_UPLOAD_QUERY,
			INSERT_OBJECT_ALLOWED_USER_QUERY,
			INSERT_OBJECT_QUERY,
			INSERT_OBJECT_STORAGE_DELETIONS_QUERY,
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
			MARK_OBJECTS_DELETE_PENDING_QUERY,
			MARK_STALE_UPLOADS_DELETE_PENDING_QUERY,
			MARK_UPLOAD_DELETE_PENDING_QUERY,
			SELECT_USERS_BY_EMAILS_QUERY,
			UPDATE_OBJECT_QUERY,
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
	deadpool::managed::Pool,
	deadpool_postgres::{
		Client,
		Manager,
	},
	jiff::Timestamp,
	rand::{
		RngExt,
		distr::Alphanumeric,
	},
	std::{
		env,
		fmt,
		time::Duration,
	},
	tokio::{
		task::JoinHandle,
		time::{
			MissedTickBehavior,
			interval,
		},
	},
	tokio_postgres::{
		Row,
		Transaction,
		error::SqlState,
	},
};

#[derive(Clone)]
pub struct ObjectLifecycleConfig {
	pending_upload_timeout_seconds: PositiveSeconds,
	storage_deletion_retry_seconds: PositiveSeconds,
	storage_deletion_lease_seconds: PositiveSeconds,
	storage_deletion_worker_interval_seconds: PositiveSeconds,
	storage_deletion_batch_size: i64,
	storage_deletion_max_attempts: i32,
}

#[derive(Clone, Copy)]
struct PositiveSeconds(i64);

impl PositiveSeconds {
	fn new(
		name: &str,
		seconds: i64,
	) -> anyhow::Result<Self> {
		if seconds <= 0 {
			anyhow::bail!("{name} must be greater than 0");
		}
		Ok(Self(seconds))
	}

	fn as_i64(self) -> i64 {
		self.0
	}

	fn as_duration(self) -> Duration {
		Duration::from_secs(self.0.unsigned_abs())
	}
}

impl ObjectLifecycleConfig {
	pub const DEFAULT_PENDING_UPLOAD_TIMEOUT_SECONDS: i64 = 3600;
	/// A reasonable default claim size for the deletion worker. The storage layer
	/// owns S3's per-request multi-delete limit and chunks internally, so this can
	/// be any positive value; 1000 happens to match S3's cap so a typical claim
	/// fires exactly one storage request.
	pub const DEFAULT_STORAGE_DELETION_BATCH_SIZE: i64 = 1000;
	pub const DEFAULT_STORAGE_DELETION_LEASE_SECONDS: i64 = 300;
	pub const DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS: i32 = 10;
	pub const DEFAULT_STORAGE_DELETION_RETRY_SECONDS: i64 = 60;
	pub const DEFAULT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS: i64 = 30;

	pub fn new(
		pending_upload_timeout_seconds: i64,
		storage_deletion_retry_seconds: i64,
		storage_deletion_lease_seconds: i64,
		storage_deletion_worker_interval_seconds: i64,
		storage_deletion_batch_size: i64,
		storage_deletion_max_attempts: i32,
	) -> anyhow::Result<Self> {
		let config = Self {
			pending_upload_timeout_seconds: PositiveSeconds::new(
				"object_upload_pending_timeout_seconds",
				pending_upload_timeout_seconds,
			)?,
			storage_deletion_retry_seconds: PositiveSeconds::new(
				"object_storage_deletion_retry_seconds",
				storage_deletion_retry_seconds,
			)?,
			storage_deletion_lease_seconds: PositiveSeconds::new(
				"object_storage_deletion_lease_seconds",
				storage_deletion_lease_seconds,
			)?,
			storage_deletion_worker_interval_seconds: PositiveSeconds::new(
				"object_storage_deletion_worker_interval_seconds",
				storage_deletion_worker_interval_seconds,
			)?,
			storage_deletion_batch_size,
			storage_deletion_max_attempts,
		};
		config.validate()?;
		Ok(config)
	}

	pub fn from_env() -> anyhow::Result<Self> {
		Self::new(
			parse_i64_env(
				"OBJECT_UPLOAD_PENDING_TIMEOUT_SECONDS",
				Self::DEFAULT_PENDING_UPLOAD_TIMEOUT_SECONDS,
			)?,
			parse_i64_env(
				"OBJECT_STORAGE_DELETION_RETRY_SECONDS",
				Self::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
			)?,
			parse_i64_env(
				"OBJECT_STORAGE_DELETION_LEASE_SECONDS",
				Self::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
			)?,
			parse_i64_env(
				"OBJECT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS",
				Self::DEFAULT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS,
			)?,
			parse_i64_env(
				"OBJECT_STORAGE_DELETION_BATCH_SIZE",
				Self::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
			)?,
			parse_i32_env(
				"OBJECT_STORAGE_DELETION_MAX_ATTEMPTS",
				Self::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
			)?,
		)
	}

	pub fn validate(&self) -> anyhow::Result<()> {
		if self.storage_deletion_batch_size <= 0 {
			anyhow::bail!("object_storage_deletion_batch_size must be greater than 0");
		}
		if self.storage_deletion_max_attempts <= 0 {
			anyhow::bail!("object_storage_deletion_max_attempts must be greater than 0");
		}
		Ok(())
	}

	fn worker_interval(&self) -> Duration {
		self.storage_deletion_worker_interval_seconds.as_duration()
	}
}

impl Default for ObjectLifecycleConfig {
	fn default() -> Self {
		Self {
			pending_upload_timeout_seconds: PositiveSeconds(
				Self::DEFAULT_PENDING_UPLOAD_TIMEOUT_SECONDS,
			),
			storage_deletion_retry_seconds: PositiveSeconds(
				Self::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
			),
			storage_deletion_lease_seconds: PositiveSeconds(
				Self::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
			),
			storage_deletion_worker_interval_seconds: PositiveSeconds(
				Self::DEFAULT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS,
			),
			storage_deletion_batch_size: Self::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
			storage_deletion_max_attempts: Self::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
		}
	}
}

impl fmt::Debug for ObjectLifecycleConfig {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		f.debug_struct("ObjectLifecycleConfig")
			.field("pending_upload_timeout_seconds", &self.pending_upload_timeout_seconds.as_i64())
			.field("storage_deletion_retry_seconds", &self.storage_deletion_retry_seconds.as_i64())
			.field("storage_deletion_lease_seconds", &self.storage_deletion_lease_seconds.as_i64())
			.field(
				"storage_deletion_worker_interval_seconds",
				&self.storage_deletion_worker_interval_seconds.as_i64(),
			)
			.field("storage_deletion_batch_size", &self.storage_deletion_batch_size)
			.field("storage_deletion_max_attempts", &self.storage_deletion_max_attempts)
			.finish()
	}
}

pub struct ObjectLifecycleService<'a> {
	db_client: &'a mut Client,
	storage: &'a StorageClient,
	config: ObjectLifecycleConfig,
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
		config: ObjectLifecycleConfig,
	) -> Self {
		Self {
			db_client,
			storage,
			config,
		}
	}

	pub async fn upload_and_create_object(
		&mut self,
		upload: ObjectUpload,
	) -> Result<S3Object, AppError> {
		let parsed_made_on = parse_made_on(upload.made_on)?;
		let location_geometry = location_geometry(upload.location.as_ref())?;
		let storage_key = generate_storage_key();
		let transaction = self.db_client.transaction().await?;
		let mut s3_object = S3Object::try_from(
			transaction
				.query_one(
					INSERT_OBJECT_QUERY,
					&[
						&upload.name,
						&storage_key,
						&upload.content_type,
						&parsed_made_on,
						&location_geometry,
						&upload.user_id,
						&upload.publicity,
					],
				)
				.await
				.map_err(|error| insert_object_error(error, &upload.name))?,
		)
		.map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message)
		})?;
		let id = s3_object.id;
		s3_object.allowed_users =
			replace_allowed_users(&transaction, id, upload.allowed_users).await?;
		transaction.commit().await?;

		if let Err(error) = self
			.storage
			.upload_object(&s3_object.storage_key, upload.bytes, upload.content_type)
			.await
		{
			if let Err(cleanup_error) =
				self.enqueue_pending_upload_cleanup(id, &s3_object.storage_key).await
			{
				tracing::error!(
					object_id = id,
					storage_key = %s3_object.storage_key,
					error = ?cleanup_error,
					"Failed to enqueue storage cleanup after upload failed"
				);
			}
			return Err(AppError::from(error));
		}

		let storage_key = s3_object.storage_key.clone();
		let allowed_users = s3_object.allowed_users;
		let finalized_row = match self
			.db_client
			.query_one(FINALIZE_OBJECT_UPLOAD_QUERY, &[&id, &storage_key])
			.await
		{
			Ok(row) => row,
			Err(error) => {
				if let Err(cleanup_error) =
					self.enqueue_pending_upload_cleanup(id, &storage_key).await
				{
					tracing::error!(
						object_id = id,
						storage_key = %storage_key,
						error = ?cleanup_error,
						"Failed to enqueue storage cleanup after upload finalization failed"
					);
				}
				return Err(AppError::from(error));
			}
		};
		let mut finalized = S3Object::try_from(finalized_row).map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message)
		})?;
		finalized.allowed_users = allowed_users;
		Ok(finalized)
	}

	pub async fn delete_objects(
		&mut self,
		ids: &[i64],
	) -> Result<Vec<S3Object>, AppError> {
		tracing::debug!("IDs to delete: {:?}", ids);
		let transaction = self.db_client.transaction().await?;
		let rows = transaction.query(MARK_OBJECTS_DELETE_PENDING_QUERY, &[&ids]).await?;
		let objects = collect_s3_objects(rows)?;
		enqueue_storage_deletions(&transaction, &objects).await?;

		transaction.commit().await?;

		Ok(objects)
	}

	/// Runs both maintenance stages in sequence, surfacing the first failure if any.
	///
	/// Each stage logs its own outcome (success count or error) so operators can tell
	/// from logs which stage struggled even when the composite returns Ok. `.and()`
	/// gives the desired "first-failure" semantics without the bookkeeping the
	/// previous shape required.
	pub async fn run_storage_maintenance(&mut self) -> Result<(), AppError> {
		let reap = self.reap_stale_pending_uploads().await;
		let drain = self.drain_storage_deletions().await;

		match &reap {
			Ok(stale_uploads) if !stale_uploads.is_empty() => tracing::warn!(
				count = stale_uploads.len(),
				"Marked stale pending uploads for storage cleanup"
			),
			Ok(_) => {}
			Err(error) => tracing::warn!(
				error = ?error,
				"Failed to mark stale pending uploads for storage cleanup"
			),
		}
		if let Err(error) = &drain {
			tracing::warn!(error = ?error, "Failed to drain object storage deletions");
		}

		reap.map(|_| ()).and(drain)
	}

	pub async fn reap_stale_pending_uploads(&mut self) -> Result<Vec<S3Object>, AppError> {
		let transaction = self.db_client.transaction().await?;
		let rows = transaction
			.query(
				MARK_STALE_UPLOADS_DELETE_PENDING_QUERY,
				&[&self.config.pending_upload_timeout_seconds.as_i64()],
			)
			.await
			.context("Failed to mark stale pending uploads for cleanup")?;
		let objects = collect_s3_objects(rows)?;
		enqueue_storage_deletions(&transaction, &objects).await?;
		transaction.commit().await?;
		Ok(objects)
	}

	pub async fn drain_storage_deletions(&mut self) -> Result<(), AppError> {
		drain_storage_deletion_outbox(self.db_client, self.storage, &self.config).await
	}

	pub async fn update_object_metadata(
		&mut self,
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

		let transaction = self.db_client.transaction().await?;
		let mut s3_object = S3Object::try_from(
			transaction
				.query_one(
					UPDATE_OBJECT_QUERY,
					&[&id, &name, &parsed_made_on, &location_geometry, &publicity],
				)
				.await?,
		)
		.map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message)
		})?;

		s3_object.allowed_users = replace_allowed_users(&transaction, id, allowed_users).await?;
		transaction.commit().await?;
		Ok(s3_object)
	}

	async fn enqueue_pending_upload_cleanup(
		&mut self,
		object_id: i64,
		storage_key: &str,
	) -> Result<(), AppError> {
		let transaction = self.db_client.transaction().await?;
		let object = S3Object::try_from(
			transaction
				.query_one(MARK_UPLOAD_DELETE_PENDING_QUERY, &[&object_id, &storage_key])
				.await
				.context("Failed to mark failed upload for storage cleanup")?,
		)
		.map_err(|e| {
			anyhow::anyhow!("Failed to convert database row to S3 object: {}", e.message)
		})?;
		enqueue_storage_deletions(&transaction, &[object]).await?;
		transaction.commit().await?;

		Ok(())
	}
}

#[derive(Clone)]
pub struct ObjectLifecycleWorker {
	pool: Pool<Manager>,
	storage: StorageClient,
	config: ObjectLifecycleConfig,
}

impl ObjectLifecycleWorker {
	pub fn new(
		pool: Pool<Manager>,
		storage: StorageClient,
		config: ObjectLifecycleConfig,
	) -> Self {
		Self {
			pool,
			storage,
			config,
		}
	}

	pub fn spawn(self) -> JoinHandle<()> {
		tokio::spawn(async move {
			self.run_forever().await;
		})
	}

	pub async fn run_once(&self) -> Result<(), AppError> {
		let mut client = self.pool.get().await?;
		let mut object_lifecycle =
			ObjectLifecycleService::new(&mut client, &self.storage, self.config.clone());
		object_lifecycle.run_storage_maintenance().await
	}

	async fn run_forever(self) {
		let mut interval = interval(self.config.worker_interval());
		interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

		loop {
			interval.tick().await;
			if let Err(error) = self.run_once().await {
				tracing::warn!(
					error = ?error,
					"Object storage lifecycle maintenance failed"
				);
			}
		}
	}
}

trait StorageDeletionSink {
	async fn delete_objects(
		&self,
		storage_keys: &[String],
	) -> anyhow::Result<()>;
}

impl StorageDeletionSink for StorageClient {
	async fn delete_objects(
		&self,
		storage_keys: &[String],
	) -> anyhow::Result<()> {
		StorageClient::delete_objects(self, storage_keys).await
	}
}

trait StorageDeletionOutbox {
	async fn claim_storage_deletions(
		&mut self,
		limit: i64,
		lease_seconds: i64,
		retry_after_seconds: i64,
		max_attempts: i32,
	) -> Result<Vec<String>, AppError>;

	async fn clear_storage_deletions(
		&mut self,
		storage_keys: &[String],
	) -> Result<(), AppError>;

	async fn mark_storage_deletions_failed(
		&mut self,
		storage_keys: &[String],
		error_message: &str,
	) -> Result<(), AppError>;
}

impl StorageDeletionOutbox for Client {
	async fn claim_storage_deletions(
		&mut self,
		limit: i64,
		lease_seconds: i64,
		retry_after_seconds: i64,
		max_attempts: i32,
	) -> Result<Vec<String>, AppError> {
		let rows = self
			.query(
				CLAIM_OBJECT_STORAGE_DELETIONS_QUERY,
				&[&limit, &lease_seconds, &retry_after_seconds, &max_attempts],
			)
			.await
			.context("Failed to claim pending object storage deletions")?;
		rows.into_iter()
			.map(|row| row.try_get("storage_key"))
			.collect::<Result<Vec<_>, _>>()
			.context("Failed to read claimed object storage deletion row")
			.map_err(AppError::from)
	}

	async fn clear_storage_deletions(
		&mut self,
		storage_keys: &[String],
	) -> Result<(), AppError> {
		let transaction = self.transaction().await?;
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

	async fn mark_storage_deletions_failed(
		&mut self,
		storage_keys: &[String],
		error_message: &str,
	) -> Result<(), AppError> {
		self.execute(MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY, &[&storage_keys, &error_message])
			.await
			.context("Failed to record object storage deletion failure")?;
		Ok(())
	}
}

/// Drains the deletion outbox to completion.
///
/// On a batch storage failure, the batch is marked failed (so it will retry on
/// a future tick or, past max_attempts, stay parked for triage) and the loop
/// continues claiming the next batch. The first error is surfaced after the
/// claim loop exhausts so the worker still sees that something went wrong.
/// Bounded by `storage_deletion_max_attempts`: a permanently broken row stops
/// being claimable once it hits the cap, so the loop cannot spin forever.
async fn drain_storage_deletion_outbox(
	outbox: &mut impl StorageDeletionOutbox,
	storage: &impl StorageDeletionSink,
	config: &ObjectLifecycleConfig,
) -> Result<(), AppError> {
	let mut first_error: Option<AppError> = None;
	loop {
		let storage_keys = outbox
			.claim_storage_deletions(
				config.storage_deletion_batch_size,
				config.storage_deletion_lease_seconds.as_i64(),
				config.storage_deletion_retry_seconds.as_i64(),
				config.storage_deletion_max_attempts,
			)
			.await?;

		if storage_keys.is_empty() {
			break;
		}

		if let Err(error) = storage.delete_objects(&storage_keys).await {
			let error_message = error.to_string();
			outbox.mark_storage_deletions_failed(&storage_keys, &error_message).await?;
			if first_error.is_none() {
				first_error = Some(AppError::Internal(error));
			}
			continue;
		}

		outbox.clear_storage_deletions(&storage_keys).await?;
	}

	match first_error {
		Some(error) => Err(error),
		None => Ok(()),
	}
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

async fn enqueue_storage_deletions(
	transaction: &Transaction<'_>,
	objects: &[S3Object],
) -> Result<(), AppError> {
	if objects.is_empty() {
		return Ok(());
	}

	let storage_keys = objects.iter().map(|object| object.storage_key.clone()).collect::<Vec<_>>();
	let object_ids = objects.iter().map(|object| object.id).collect::<Vec<_>>();
	transaction
		.execute(INSERT_OBJECT_STORAGE_DELETIONS_QUERY, &[&storage_keys, &object_ids])
		.await
		.context("Failed to enqueue object storage deletions")?;
	Ok(())
}

fn collect_s3_objects(rows: Vec<Row>) -> Result<Vec<S3Object>, AppError> {
	rows.into_iter().map(S3Object::try_from).collect::<Result<Vec<_>, _>>().map_err(|e| {
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

fn generate_storage_key() -> String {
	let key: String = rand::rng().sample_iter(Alphanumeric).take(40).map(char::from).collect();
	format!("objects/{key}")
}

fn insert_object_error(
	error: tokio_postgres::Error,
	name: &str,
) -> AppError {
	if error.as_db_error().is_some_and(|db_error| {
		db_error.code() == &SqlState::UNIQUE_VIOLATION &&
			matches!(db_error.constraint(), Some("objects_active_name_key" | "objects_name_key"))
	}) {
		return AppError::Validation(format!("Object named '{name}' already exists"));
	}

	AppError::from(error)
}

fn parse_i64_env(
	name: &str,
	default: i64,
) -> anyhow::Result<i64> {
	env::var(name)
		.unwrap_or_else(|_| default.to_string())
		.parse()
		.with_context(|| format!("{name} must be an integer"))
}

fn parse_i32_env(
	name: &str,
	default: i32,
) -> anyhow::Result<i32> {
	env::var(name)
		.unwrap_or_else(|_| default.to_string())
		.parse()
		.with_context(|| format!("{name} must be a 32-bit integer"))
}

#[cfg(test)]
mod tests {
	use {
		super::{
			AppError,
			ObjectLifecycleConfig,
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
		requested_leases: Vec<i64>,
		requested_retries: Vec<i64>,
		requested_max_attempts: Vec<i32>,
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
		async fn claim_storage_deletions(
			&mut self,
			limit: i64,
			lease_seconds: i64,
			retry_after_seconds: i64,
			max_attempts: i32,
		) -> Result<Vec<String>, AppError> {
			self.requested_limits.push(limit);
			self.requested_leases.push(lease_seconds);
			self.requested_retries.push(retry_after_seconds);
			self.requested_max_attempts.push(max_attempts);
			Ok(self.pending_batches.pop_front().unwrap_or_default())
		}

		async fn clear_storage_deletions(
			&mut self,
			storage_keys: &[String],
		) -> Result<(), AppError> {
			self.cleared_batches.push(storage_keys.to_vec());
			Ok(())
		}

		async fn mark_storage_deletions_failed(
			&mut self,
			storage_keys: &[String],
			error_message: &str,
		) -> Result<(), AppError> {
			self.failed_batches.push((storage_keys.to_vec(), error_message.to_string()));
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
			storage_keys: &[String],
		) -> anyhow::Result<()> {
			if let Some(error_message) = self.error_message {
				anyhow::bail!(error_message);
			}

			let mut deleted_batches = self
				.deleted_batches
				.lock()
				.map_err(|_| anyhow::anyhow!("deleted batch lock is poisoned"))?;
			deleted_batches.push(storage_keys.to_vec());
			Ok(())
		}
	}

	#[tokio::test]
	async fn drain_storage_deletion_outbox_clears_all_pending_batches() -> anyhow::Result<()> {
		let config = ObjectLifecycleConfig::default();
		let first_batch = storage_keys(
			"first",
			ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE as usize,
		);
		let second_batch = storage_keys("second", 2);
		let mut outbox = FakeStorageDeletionOutbox::with_pending_batches(vec![
			first_batch.clone(),
			second_batch.clone(),
		]);
		let storage = FakeStorageDeletionSink::default();

		drain_storage_deletion_outbox(&mut outbox, &storage, &config).await?;

		assert_eq!(
			outbox.requested_limits,
			vec![
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
			]
		);
		assert_eq!(
			outbox.requested_leases,
			vec![
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
			]
		);
		assert_eq!(
			outbox.requested_retries,
			vec![
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
			]
		);
		assert_eq!(storage.deleted_batches()?, vec![first_batch.clone(), second_batch.clone()]);
		assert_eq!(outbox.cleared_batches, vec![first_batch, second_batch]);
		assert!(outbox.failed_batches.is_empty());
		assert_eq!(
			outbox.requested_max_attempts,
			vec![
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
			]
		);

		Ok(())
	}

	#[tokio::test]
	async fn drain_storage_deletion_outbox_records_storage_delete_failure() -> anyhow::Result<()> {
		let config = ObjectLifecycleConfig::default();
		let pending_batch = storage_keys("failed", 2);
		let mut outbox =
			FakeStorageDeletionOutbox::with_pending_batches(vec![pending_batch.clone()]);
		let storage = FakeStorageDeletionSink::failing("storage delete failed");

		let Err(error) = drain_storage_deletion_outbox(&mut outbox, &storage, &config).await else {
			anyhow::bail!("storage delete failure should surface as an error");
		};

		assert!(matches!(error, AppError::Internal(_)));
		assert!(storage.deleted_batches()?.is_empty());
		assert!(outbox.cleared_batches.is_empty());
		assert_eq!(
			outbox.failed_batches,
			vec![(pending_batch, "storage delete failed".to_string())]
		);
		// Two claims: one returning the failing batch, then one returning empty after
		// the batch is marked failed. Drain continues past failures rather than aborting
		// on the first; the second claim drains the queue to completion.
		assert_eq!(
			outbox.requested_limits,
			vec![
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
			]
		);

		Ok(())
	}

	#[test]
	fn object_lifecycle_config_rejects_invalid_values() {
		let valid = ObjectLifecycleConfig::default();
		assert!(valid.validate().is_ok());
		assert!(
			ObjectLifecycleConfig::new(
				i64::MAX,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
			)
			.is_ok()
		);

		assert!(
			ObjectLifecycleConfig::new(
				0,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_RETRY_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
			)
			.is_err()
		);
		assert!(
			ObjectLifecycleConfig::new(
				ObjectLifecycleConfig::DEFAULT_PENDING_UPLOAD_TIMEOUT_SECONDS,
				-1,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_LEASE_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_WORKER_INTERVAL_SECONDS,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_BATCH_SIZE,
				ObjectLifecycleConfig::DEFAULT_STORAGE_DELETION_MAX_ATTEMPTS,
			)
			.is_err()
		);

		let invalid_batch = ObjectLifecycleConfig {
			storage_deletion_batch_size: 0,
			..valid.clone()
		};
		assert!(invalid_batch.validate().is_err());

		let invalid_max_attempts = ObjectLifecycleConfig {
			storage_deletion_max_attempts: 0,
			..valid
		};
		assert!(invalid_max_attempts.validate().is_err());
	}

	fn storage_keys(
		prefix: &str,
		count: usize,
	) -> Vec<String> {
		(0 .. count).map(|index| format!("{prefix}-{index}")).collect()
	}
}
