use {
	crate::{
		db::queries::{
			CLAIM_EXPIRED_OBJECT_UPLOAD_SESSIONS_QUERY,
			CLAIM_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECT_ALLOWED_USERS_QUERY,
			DELETE_OBJECT_STORAGE_DELETIONS_QUERY,
			DELETE_OBJECT_UPLOAD_SESSION_QUERY,
			DELETE_OBJECTS_BY_STORAGE_KEYS_QUERY,
			DELETE_PENDING_OBJECT_UPLOAD_BY_SESSION_QUERY,
			DELETE_PENDING_OBJECT_UPLOAD_QUERY,
			FINALIZE_OBJECT_UPLOAD_QUERY,
			INSERT_OBJECT_ALLOWED_USER_QUERY,
			INSERT_OBJECT_QUERY,
			INSERT_OBJECT_STORAGE_DELETIONS_QUERY,
			INSERT_OBJECT_UPLOAD_SESSION_QUERY,
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
			MARK_OBJECT_UPLOAD_SESSION_CLEANUP_FAILED_QUERY,
			MARK_OBJECTS_DELETE_PENDING_QUERY,
			MARK_STALE_UPLOADS_DELETE_PENDING_QUERY,
			MARK_UPLOAD_DELETE_PENDING_QUERY,
			SELECT_ACTIVE_OBJECT_UPLOAD_SESSION_FOR_USER_QUERY,
			SELECT_AVAILABLE_OBJECT_FOR_USER_QUERY,
			SELECT_OBJECT_UPLOAD_SESSION_FOR_USER_QUERY,
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
		storage::{
			CompletedUploadPart,
			MultipartUploadAbortOutcome,
			MultipartUploadCompleteOutcome,
			PresignedHeader,
			StorageClient,
			StoredObjectMetadata,
		},
	},
	anyhow::Context,
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
	serde::Deserialize,
	shared::ALLOWED_MIME_TYPES,
	std::{
		collections::BTreeSet,
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
	#[serde(default = "ObjectLifecycleConfig::default_storage_deletion_worker_interval_seconds")]
	pub storage_deletion_worker_interval_seconds: i64,
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

	pub const fn default_storage_deletion_worker_interval_seconds() -> i64 {
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
		if self.pending_upload_timeout_seconds <= 0 {
			anyhow::bail!("pending_upload_timeout_seconds must be greater than 0");
		}
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
		if self.upload_max_part_count <= 0 {
			anyhow::bail!("upload_max_part_count must be greater than 0");
		}
		if self.upload_max_part_count > Self::S3_MAX_MULTIPART_PART_COUNT {
			anyhow::bail!(
				"upload_max_part_count must be at most {}",
				Self::S3_MAX_MULTIPART_PART_COUNT
			);
		}
		if self.upload_session_ttl_seconds <= 0 {
			anyhow::bail!("upload_session_ttl_seconds must be greater than 0");
		}
		if self.upload_session_cleanup_retry_seconds <= 0 {
			anyhow::bail!("upload_session_cleanup_retry_seconds must be greater than 0");
		}
		if self.upload_session_cleanup_lease_seconds <= 0 {
			anyhow::bail!("upload_session_cleanup_lease_seconds must be greater than 0");
		}
		if self.upload_session_cleanup_max_attempts <= 0 {
			anyhow::bail!("upload_session_cleanup_max_attempts must be greater than 0");
		}
		if self.upload_session_cleanup_batch_size <= 0 {
			anyhow::bail!("upload_session_cleanup_batch_size must be greater than 0");
		}
		self.upload_session_total_parts(self.upload_max_file_size_bytes)?;
		if self.storage_deletion_retry_seconds <= 0 {
			anyhow::bail!("storage_deletion_retry_seconds must be greater than 0");
		}
		if self.storage_deletion_lease_seconds <= 0 {
			anyhow::bail!("storage_deletion_lease_seconds must be greater than 0");
		}
		if self.storage_deletion_worker_interval_seconds <= 0 {
			anyhow::bail!("storage_deletion_worker_interval_seconds must be greater than 0");
		}
		if self.storage_deletion_batch_size <= 0 {
			anyhow::bail!("storage_deletion_batch_size must be greater than 0");
		}
		if self.storage_deletion_max_attempts <= 0 {
			anyhow::bail!("storage_deletion_max_attempts must be greater than 0");
		}
		Ok(())
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
		let part_count = ((file_size_bytes - 1) / self.upload_part_size_bytes) + 1;
		if part_count > i64::from(self.upload_max_part_count) {
			anyhow::bail!(
				"file_size_bytes requires {part_count} parts, exceeding upload_max_part_count {}",
				self.upload_max_part_count
			);
		}
		i32::try_from(part_count).context("upload part count exceeds i32 range")
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
		let part_start = i64::from(part_number - 1) * self.upload_part_size_bytes;
		Ok(self.upload_part_size_bytes.min(file_size_bytes - part_start))
	}

	/// Returns Self if `validate` succeeds; convenient for builder-style construction.
	pub fn validated(self) -> anyhow::Result<Self> {
		self.validate()?;
		Ok(self)
	}

	fn worker_interval(&self) -> Duration {
		Duration::from_secs(self.storage_deletion_worker_interval_seconds.unsigned_abs())
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
			storage_deletion_worker_interval_seconds:
				Self::default_storage_deletion_worker_interval_seconds(),
			storage_deletion_batch_size: Self::default_storage_deletion_batch_size(),
			storage_deletion_max_attempts: Self::default_storage_deletion_max_attempts(),
		}
	}
}

pub struct ObjectLifecycleService<'a> {
	db_client: &'a mut Client,
	storage: &'a StorageClient,
	config: ObjectLifecycleConfig,
}

pub struct ObjectUploadSessionCreate {
	pub name: String,
	pub content_type: String,
	pub file_size_bytes: i64,
	pub made_on: Option<String>,
	pub location: Option<Location>,
	pub user_id: i64,
	pub publicity: PublicityOverride,
	pub allowed_users: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct CreatedObjectUploadSession {
	pub object_id: i64,
	pub part_size_bytes: i64,
	pub total_parts: i32,
	pub expires_at: Timestamp,
}

#[derive(Clone, Debug)]
struct ObjectUploadSession {
	object_id: i64,
	storage_key: String,
	upload_id: String,
	content_type: String,
	file_size_bytes: i64,
	part_size_bytes: i64,
	expires_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct PresignedObjectUploadPart {
	pub part_number: i32,
	pub url: String,
	pub method: String,
	pub headers: Vec<PresignedHeader>,
	pub expected_content_length: i64,
}

const MAX_PRESIGN_PARTS_PER_REQUEST: usize = 100;

impl TryFrom<Row> for ObjectUploadSession {
	type Error = AppError;

	fn try_from(row: Row) -> Result<Self, Self::Error> {
		Ok(Self {
			object_id: row
				.try_get("object_id")
				.context("Failed to read upload session object_id")?,
			storage_key: row
				.try_get("storage_key")
				.context("Failed to read upload session storage_key")?,
			upload_id: row
				.try_get("upload_id")
				.context("Failed to read upload session upload_id")?,
			content_type: row
				.try_get("content_type")
				.context("Failed to read upload session content_type")?,
			file_size_bytes: row
				.try_get("file_size")
				.context("Failed to read upload session file_size")?,
			part_size_bytes: row
				.try_get("part_size_bytes")
				.context("Failed to read upload session part_size_bytes")?,
			expires_at: row
				.try_get("expires_at")
				.context("Failed to read upload session expires_at")?,
		})
	}
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

	pub async fn create_upload_session(
		&mut self,
		upload: ObjectUploadSessionCreate,
	) -> Result<CreatedObjectUploadSession, AppError> {
		validate_upload_name(&upload.name)?;
		validate_upload_content_type(&upload.content_type)?;
		let parsed_made_on = parse_made_on(upload.made_on.clone())?;
		let location_geometry = location_geometry(upload.location.as_ref())?;
		let total_parts = self.config.upload_session_total_parts(upload.file_size_bytes)?;
		let part_size_bytes = self.config.upload_part_size_bytes;
		let session_ttl_seconds = self.config.upload_session_ttl_seconds;
		let allowed_users = upload.allowed_users.clone();
		let storage_key = generate_storage_key();
		let upload_id = self
			.storage
			.create_multipart_upload(&storage_key, &upload.content_type)
			.await
			.context("Failed to create object upload session in storage")?;

		let session_result = async {
			let transaction = self.db_client.transaction().await?;
			let object_id = insert_pending_upload_object(
				&transaction,
				&upload,
				&storage_key,
				&parsed_made_on,
				&location_geometry,
			)
			.await?;
			replace_allowed_users(&transaction, object_id, allowed_users).await?;
			let session = insert_object_upload_session(
				&transaction,
				object_id,
				&storage_key,
				&upload_id,
				&upload,
				part_size_bytes,
				session_ttl_seconds,
			)
			.await?;
			transaction.commit().await?;
			Ok::<_, AppError>(session)
		}
		.await;
		let session = match session_result {
			Ok(session) => session,
			Err(error) => {
				abort_created_multipart_upload(self.storage, &storage_key, &upload_id, &error)
					.await;
				return Err(error);
			}
		};

		Ok(CreatedObjectUploadSession {
			object_id: session.object_id,
			part_size_bytes: session.part_size_bytes,
			total_parts,
			expires_at: session.expires_at,
		})
	}

	pub async fn presign_upload_parts(
		&mut self,
		object_id: i64,
		user_id: i64,
		part_numbers: Vec<i32>,
	) -> Result<Vec<PresignedObjectUploadPart>, AppError> {
		let session = self.active_upload_session_for_user(object_id, user_id).await?;
		validate_part_numbers(&session, &part_numbers)?;

		let mut presigned_parts = Vec::with_capacity(part_numbers.len());
		for part_number in part_numbers {
			let expected_content_length = upload_session_part_size(&session, part_number)?;
			let presigned = self
				.storage
				.presigned_upload_part_url(
					&session.storage_key,
					&session.upload_id,
					part_number,
					expected_content_length,
				)
				.await
				.context("Failed to presign object upload part")?;
			presigned_parts.push(PresignedObjectUploadPart {
				part_number,
				url: presigned.url,
				method: presigned.method,
				headers: presigned.headers,
				expected_content_length: presigned.expected_content_length,
			});
		}

		Ok(presigned_parts)
	}

	pub async fn complete_upload(
		&mut self,
		object_id: i64,
		user_id: i64,
		completed_parts: Vec<CompletedUploadPart>,
	) -> Result<S3Object, AppError> {
		if let Some(object) = self.available_object_for_user(object_id, user_id).await? {
			return Ok(object);
		}

		let session = self.active_upload_session_for_user(object_id, user_id).await?;
		let completed_parts = validate_completed_parts(&session, completed_parts)?;

		match self
			.storage
			.complete_multipart_upload(&session.storage_key, &session.upload_id, &completed_parts)
			.await?
		{
			MultipartUploadCompleteOutcome::Completed => {}
			MultipartUploadCompleteOutcome::UploadNotFound =>
				return match self.storage.head_object_opt(&session.storage_key).await.context(
					"Failed to check completed object after multipart upload was missing",
				)? {
					Some(metadata) =>
						self.finalize_verified_upload_session(&session, user_id, metadata).await,
					None => Err(AppError::NotFound("Multipart upload not found".to_string())),
				},
		}

		let metadata = self
			.storage
			.head_object(&session.storage_key)
			.await
			.context("Failed to verify completed object metadata")?;
		self.finalize_verified_upload_session(&session, user_id, metadata).await
	}

	pub async fn abort_upload(
		&mut self,
		object_id: i64,
		user_id: i64,
	) -> Result<(), AppError> {
		let session = self.upload_session_for_user(object_id, user_id).await?;

		match self.storage.abort_multipart_upload(&session.storage_key, &session.upload_id).await? {
			MultipartUploadAbortOutcome::Aborted =>
				self.delete_pending_upload_metadata(&session, user_id).await,
			MultipartUploadAbortOutcome::UploadNotFound => match self
				.storage
				.head_object_opt(&session.storage_key)
				.await
				.context("Failed to check object after multipart upload was missing")?
			{
				Some(_) => self.enqueue_completed_upload_cleanup(&session).await,
				None => self.delete_pending_upload_metadata(&session, user_id).await,
			},
		}
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

	/// Runs maintenance stages in sequence, surfacing the first failure if any.
	///
	/// Each stage logs its own outcome (success count or error) so operators can tell
	/// from logs which stage struggled even when the composite returns Ok. `.and()`
	/// gives the desired "first-failure" semantics without the bookkeeping the
	/// previous shape required.
	pub async fn run_storage_maintenance(&mut self) -> Result<(), AppError> {
		let reconcile = self.reconcile_expired_upload_sessions().await;
		let reap = self.reap_stale_pending_uploads().await;
		let drain = self.drain_storage_deletions().await;

		match &reconcile {
			Ok(reconciled_uploads) if *reconciled_uploads > 0 => tracing::warn!(
				count = reconciled_uploads,
				"Reconciled expired object upload sessions"
			),
			Ok(_) => {}
			Err(error) => tracing::warn!(
				error = ?error,
				"Failed to reconcile expired object upload sessions"
			),
		}
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

		reconcile.map(|_| ()).and(reap.map(|_| ())).and(drain)
	}

	pub async fn reconcile_expired_upload_sessions(&mut self) -> Result<usize, AppError> {
		let mut reconciled_count = 0;
		let mut first_error: Option<AppError> = None;

		loop {
			let sessions = self.claim_expired_upload_sessions().await?;
			if sessions.is_empty() {
				break;
			}

			for session in sessions {
				match self.reconcile_expired_upload_session(&session).await {
					Ok(()) => reconciled_count += 1,
					Err(error) => {
						let error_message = error.to_string();
						if let Err(mark_error) = self
							.mark_upload_session_cleanup_failed(session.object_id, &error_message)
							.await
						{
							if first_error.is_none() {
								first_error = Some(mark_error);
							}
							continue;
						}
						if first_error.is_none() {
							first_error = Some(error);
						}
					}
				}
			}
		}

		match first_error {
			Some(error) => Err(error),
			None => Ok(reconciled_count),
		}
	}

	pub async fn reap_stale_pending_uploads(&mut self) -> Result<Vec<S3Object>, AppError> {
		let transaction = self.db_client.transaction().await?;
		let rows = transaction
			.query(
				MARK_STALE_UPLOADS_DELETE_PENDING_QUERY,
				&[&self.config.pending_upload_timeout_seconds],
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

	async fn claim_expired_upload_sessions(
		&mut self
	) -> Result<Vec<ObjectUploadSession>, AppError> {
		let rows = self
			.db_client
			.query(
				CLAIM_EXPIRED_OBJECT_UPLOAD_SESSIONS_QUERY,
				&[
					&self.config.upload_session_cleanup_batch_size,
					&self.config.upload_session_cleanup_lease_seconds,
					&self.config.upload_session_cleanup_max_attempts,
				],
			)
			.await
			.context("Failed to claim expired object upload sessions")?;
		rows.into_iter().map(ObjectUploadSession::try_from).collect()
	}

	async fn reconcile_expired_upload_session(
		&mut self,
		session: &ObjectUploadSession,
	) -> Result<(), AppError> {
		match self
			.storage
			.abort_multipart_upload(&session.storage_key, &session.upload_id)
			.await
			.context("Failed to abort expired multipart upload")?
		{
			MultipartUploadAbortOutcome::Aborted =>
				self.delete_pending_upload_metadata_for_cleanup(session).await,
			MultipartUploadAbortOutcome::UploadNotFound => match self
				.storage
				.head_object_opt(&session.storage_key)
				.await
				.context("Failed to check object after expired multipart upload was missing")?
			{
				Some(_) => self.enqueue_completed_upload_cleanup(session).await,
				None => self.delete_pending_upload_metadata_for_cleanup(session).await,
			},
		}
	}

	async fn mark_upload_session_cleanup_failed(
		&mut self,
		object_id: i64,
		error_message: &str,
	) -> Result<(), AppError> {
		self.db_client
			.execute(
				MARK_OBJECT_UPLOAD_SESSION_CLEANUP_FAILED_QUERY,
				&[&object_id, &error_message, &self.config.upload_session_cleanup_retry_seconds],
			)
			.await
			.context("Failed to record object upload session cleanup failure")?;
		Ok(())
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

	async fn active_upload_session_for_user(
		&mut self,
		object_id: i64,
		user_id: i64,
	) -> Result<ObjectUploadSession, AppError> {
		self.db_client
			.query_opt(SELECT_ACTIVE_OBJECT_UPLOAD_SESSION_FOR_USER_QUERY, &[&object_id, &user_id])
			.await
			.context("Failed to load active object upload session")?
			.map(ObjectUploadSession::try_from)
			.transpose()?
			.ok_or_else(|| AppError::NotFound("Upload session not found".to_string()))
	}

	async fn upload_session_for_user(
		&mut self,
		object_id: i64,
		user_id: i64,
	) -> Result<ObjectUploadSession, AppError> {
		self.db_client
			.query_opt(SELECT_OBJECT_UPLOAD_SESSION_FOR_USER_QUERY, &[&object_id, &user_id])
			.await
			.context("Failed to load object upload session")?
			.map(ObjectUploadSession::try_from)
			.transpose()?
			.ok_or_else(|| AppError::NotFound("Upload session not found".to_string()))
	}

	async fn available_object_for_user(
		&mut self,
		object_id: i64,
		user_id: i64,
	) -> Result<Option<S3Object>, AppError> {
		self.db_client
			.query_opt(SELECT_AVAILABLE_OBJECT_FOR_USER_QUERY, &[&object_id, &user_id])
			.await
			.context("Failed to load available object")?
			.map(s3_object_from_row)
			.transpose()
	}

	async fn finalize_verified_upload_session(
		&mut self,
		session: &ObjectUploadSession,
		user_id: i64,
		metadata: StoredObjectMetadata,
	) -> Result<S3Object, AppError> {
		if let Some(error_message) = completed_upload_metadata_error(session, &metadata) {
			self.enqueue_completed_upload_cleanup(session).await?;
			return Err(AppError::Validation(error_message));
		}

		self.finalize_upload_session(session, user_id).await
	}

	async fn finalize_upload_session(
		&mut self,
		session: &ObjectUploadSession,
		user_id: i64,
	) -> Result<S3Object, AppError> {
		let transaction = self.db_client.transaction().await?;
		let finalized = transaction
			.query_opt(FINALIZE_OBJECT_UPLOAD_QUERY, &[&session.object_id, &session.storage_key])
			.await
			.context("Failed to finalize object upload")?;
		let Some(finalized) = finalized else {
			drop(transaction);
			return self
				.available_object_for_user(session.object_id, user_id)
				.await?
				.ok_or_else(|| AppError::NotFound("Upload session not found".to_string()));
		};
		transaction
			.execute(DELETE_OBJECT_UPLOAD_SESSION_QUERY, &[&session.object_id])
			.await
			.context("Failed to delete finalized object upload session")?;
		transaction.commit().await?;
		s3_object_from_row(finalized)
	}

	async fn delete_pending_upload_metadata(
		&mut self,
		session: &ObjectUploadSession,
		user_id: i64,
	) -> Result<(), AppError> {
		self.db_client
			.execute(
				DELETE_PENDING_OBJECT_UPLOAD_QUERY,
				&[&session.object_id, &session.storage_key, &user_id],
			)
			.await
			.context("Failed to delete aborted object upload metadata")?;
		Ok(())
	}

	async fn delete_pending_upload_metadata_for_cleanup(
		&mut self,
		session: &ObjectUploadSession,
	) -> Result<(), AppError> {
		self.db_client
			.execute(
				DELETE_PENDING_OBJECT_UPLOAD_BY_SESSION_QUERY,
				&[&session.object_id, &session.storage_key],
			)
			.await
			.context("Failed to delete expired object upload metadata")?;
		Ok(())
	}

	async fn enqueue_completed_upload_cleanup(
		&mut self,
		session: &ObjectUploadSession,
	) -> Result<(), AppError> {
		let transaction = self.db_client.transaction().await?;
		let object = s3_object_from_row(
			transaction
				.query_one(
					MARK_UPLOAD_DELETE_PENDING_QUERY,
					&[&session.object_id, &session.storage_key],
				)
				.await
				.context("Failed to mark completed upload for storage cleanup")?,
		)?;
		transaction
			.execute(DELETE_OBJECT_UPLOAD_SESSION_QUERY, &[&session.object_id])
			.await
			.context("Failed to delete object upload session queued for cleanup")?;
		enqueue_storage_deletions(&transaction, &[object]).await?;
		transaction.commit().await?;
		Ok(())
	}
}

async fn abort_created_multipart_upload(
	storage: &StorageClient,
	storage_key: &str,
	upload_id: &str,
	source_error: &AppError,
) {
	if let Err(abort_error) = storage.abort_multipart_upload(storage_key, upload_id).await {
		tracing::error!(
			storage_key = %storage_key,
			upload_id = %upload_id,
			source_error = ?source_error,
			error = ?abort_error,
			"Failed to abort multipart upload after upload-session creation failed"
		);
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
		retry_after_seconds: i64,
	) -> Result<(), AppError>;
}

impl StorageDeletionOutbox for Client {
	async fn claim_storage_deletions(
		&mut self,
		limit: i64,
		lease_seconds: i64,
		max_attempts: i32,
	) -> Result<Vec<String>, AppError> {
		let rows = self
			.query(CLAIM_OBJECT_STORAGE_DELETIONS_QUERY, &[&limit, &lease_seconds, &max_attempts])
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
		retry_after_seconds: i64,
	) -> Result<(), AppError> {
		self.execute(
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
			&[&storage_keys, &error_message, &retry_after_seconds],
		)
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
				config.storage_deletion_lease_seconds,
				config.storage_deletion_max_attempts,
			)
			.await?;

		if storage_keys.is_empty() {
			break;
		}

		if let Err(error) = storage.delete_objects(&storage_keys).await {
			let error_message = error.to_string();
			outbox
				.mark_storage_deletions_failed(
					&storage_keys,
					&error_message,
					config.storage_deletion_retry_seconds,
				)
				.await?;
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

async fn insert_pending_upload_object(
	transaction: &Transaction<'_>,
	upload: &ObjectUploadSessionCreate,
	storage_key: &str,
	parsed_made_on: &Option<Timestamp>,
	location_geometry: &Option<String>,
) -> Result<i64, AppError> {
	let row = transaction
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
		.map_err(|error| insert_object_error(error, &upload.name))?;
	row.try_get("id").context("Failed to read inserted object id").map_err(AppError::from)
}

async fn insert_object_upload_session(
	transaction: &Transaction<'_>,
	object_id: i64,
	storage_key: &str,
	upload_id: &str,
	upload: &ObjectUploadSessionCreate,
	part_size_bytes: i64,
	session_ttl_seconds: i64,
) -> Result<ObjectUploadSession, AppError> {
	let row = transaction
		.query_one(
			INSERT_OBJECT_UPLOAD_SESSION_QUERY,
			&[
				&object_id,
				&storage_key,
				&upload_id,
				&upload.content_type,
				&upload.file_size_bytes,
				&part_size_bytes,
				&session_ttl_seconds,
			],
		)
		.await
		.context("Failed to insert object upload session")?;
	ObjectUploadSession::try_from(row)
}

fn validate_upload_name(name: &str) -> Result<(), AppError> {
	if name.trim().is_empty() {
		return Err(AppError::Validation("Object name must not be empty".to_string()));
	}
	Ok(())
}

fn validate_upload_content_type(content_type: &str) -> Result<(), AppError> {
	if ALLOWED_MIME_TYPES.contains(&content_type) {
		return Ok(());
	}
	Err(AppError::Validation(format!("Unsupported file type: {content_type}")))
}

fn validate_part_numbers(
	session: &ObjectUploadSession,
	part_numbers: &[i32],
) -> Result<(), AppError> {
	if part_numbers.is_empty() {
		return Err(AppError::Validation("part_numbers must not be empty".to_string()));
	}
	if part_numbers.len() > MAX_PRESIGN_PARTS_PER_REQUEST {
		return Err(AppError::Validation(format!(
			"part_numbers may contain at most {MAX_PRESIGN_PARTS_PER_REQUEST} entries"
		)));
	}

	let total_parts = upload_session_total_parts(session)?;
	let mut seen = BTreeSet::new();
	for part_number in part_numbers {
		if !(1 ..= total_parts).contains(part_number) {
			return Err(AppError::Validation(format!(
				"part_number must be between 1 and {total_parts}"
			)));
		}
		if !seen.insert(*part_number) {
			return Err(AppError::Validation(format!(
				"part_number {part_number} was requested more than once"
			)));
		}
	}

	Ok(())
}

fn upload_session_total_parts(session: &ObjectUploadSession) -> Result<i32, AppError> {
	let part_count = ((session.file_size_bytes - 1) / session.part_size_bytes) + 1;
	if part_count > i64::from(ObjectLifecycleConfig::S3_MAX_MULTIPART_PART_COUNT) {
		return Err(AppError::Validation(format!(
			"upload session requires {part_count} parts, exceeding S3 multipart limit {}",
			ObjectLifecycleConfig::S3_MAX_MULTIPART_PART_COUNT
		)));
	}
	i32::try_from(part_count)
		.context("upload session part count exceeds i32 range")
		.map_err(AppError::from)
}

fn upload_session_part_size(
	session: &ObjectUploadSession,
	part_number: i32,
) -> Result<i64, AppError> {
	let total_parts = upload_session_total_parts(session)?;
	if !(1 ..= total_parts).contains(&part_number) {
		return Err(AppError::Validation(format!(
			"part_number must be between 1 and {total_parts}"
		)));
	}
	let part_start = i64::from(part_number - 1) * session.part_size_bytes;
	Ok(session.part_size_bytes.min(session.file_size_bytes - part_start))
}

fn validate_completed_parts(
	session: &ObjectUploadSession,
	mut completed_parts: Vec<CompletedUploadPart>,
) -> Result<Vec<CompletedUploadPart>, AppError> {
	let total_parts = upload_session_total_parts(session)?;
	if completed_parts.len() != total_parts as usize {
		return Err(AppError::Validation(format!(
			"completed parts must contain exactly {total_parts} entries"
		)));
	}

	completed_parts.sort_by_key(|part| part.part_number);
	for (index, part) in completed_parts.iter().enumerate() {
		if part.e_tag.trim().is_empty() {
			return Err(AppError::Validation(format!(
				"completed part {} must include a non-empty ETag",
				part.part_number
			)));
		}
		let expected_part_number = i32::try_from(index + 1)
			.context("completed part index exceeds i32 range")
			.map_err(AppError::from)?;
		if part.part_number != expected_part_number {
			return Err(AppError::Validation(format!(
				"completed parts must include every part number from 1 to {total_parts} exactly once"
			)));
		}
	}

	Ok(completed_parts)
}

fn completed_upload_metadata_error(
	session: &ObjectUploadSession,
	metadata: &StoredObjectMetadata,
) -> Option<String> {
	if metadata.content_length != session.file_size_bytes {
		return Some("Completed upload size did not match declared file size".to_string());
	}
	if metadata.content_type != session.content_type {
		return Some(
			"Completed upload content type did not match declared content type".to_string(),
		);
	}
	None
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

fn s3_object_from_row(row: Row) -> Result<S3Object, AppError> {
	S3Object::try_from(row).map_err(|e| {
		AppError::Internal(anyhow::anyhow!(
			"Failed to convert database row to S3 object: {}",
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

/// Generates an unguessable storage key used directly in presigned S3 URLs.
///
/// MUST use a cryptographically secure RNG. `rand::rng()` (the default
/// `ThreadRng`) is documented as a CSPRNG; do not switch to `SmallRng`,
/// `StdRng::from_seed` with a guessable seed, or any non-cryptographic
/// generator, or presigned URLs become enumerable.
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

#[cfg(test)]
mod tests {
	use {
		super::{
			AppError,
			ObjectLifecycleConfig,
			ObjectUploadSession,
			StorageDeletionOutbox,
			StorageDeletionSink,
			completed_upload_metadata_error,
			drain_storage_deletion_outbox,
			validate_completed_parts,
		},
		crate::storage::{
			CompletedUploadPart,
			StoredObjectMetadata,
		},
		jiff::Timestamp,
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
		requested_max_attempts: Vec<i32>,
		cleared_batches: Vec<Vec<String>>,
		failed_batches: Vec<(Vec<String>, String, i64)>,
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
			max_attempts: i32,
		) -> Result<Vec<String>, AppError> {
			self.requested_limits.push(limit);
			self.requested_leases.push(lease_seconds);
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
			retry_after_seconds: i64,
		) -> Result<(), AppError> {
			self.failed_batches.push((
				storage_keys.to_vec(),
				error_message.to_string(),
				retry_after_seconds,
			));
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
		let first_batch = storage_keys("first", config.storage_deletion_batch_size as usize);
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
				config.storage_deletion_batch_size,
				config.storage_deletion_batch_size,
				config.storage_deletion_batch_size,
			]
		);
		assert_eq!(
			outbox.requested_leases,
			vec![
				config.storage_deletion_lease_seconds,
				config.storage_deletion_lease_seconds,
				config.storage_deletion_lease_seconds,
			]
		);
		assert_eq!(storage.deleted_batches()?, vec![first_batch.clone(), second_batch.clone()]);
		assert_eq!(outbox.cleared_batches, vec![first_batch, second_batch]);
		assert!(outbox.failed_batches.is_empty());
		assert_eq!(
			outbox.requested_max_attempts,
			vec![
				config.storage_deletion_max_attempts,
				config.storage_deletion_max_attempts,
				config.storage_deletion_max_attempts,
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
			vec![(
				pending_batch,
				"storage delete failed".to_string(),
				config.storage_deletion_retry_seconds
			)]
		);
		// Two claims: one returning the failing batch, then one returning empty after
		// the batch is marked failed. Drain continues past failures rather than aborting
		// on the first; the second claim drains the queue to completion.
		assert_eq!(
			outbox.requested_limits,
			vec![config.storage_deletion_batch_size, config.storage_deletion_batch_size,]
		);

		Ok(())
	}

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
				storage_deletion_worker_interval_seconds: -5,
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

	#[test]
	fn completed_parts_validation_requires_exact_sequential_parts() -> anyhow::Result<()> {
		let session = upload_session_for_test(
			(ObjectLifecycleConfig::S3_MIN_MULTIPART_PART_SIZE_BYTES * 2) - 1,
			ObjectLifecycleConfig::S3_MIN_MULTIPART_PART_SIZE_BYTES,
		);

		let sorted = validate_completed_parts(
			&session,
			vec![completed_part(2, "etag-2"), completed_part(1, "etag-1")],
		)?;

		assert_eq!(sorted.iter().map(|part| part.part_number).collect::<Vec<_>>(), vec![1, 2]);
		assert!(validate_completed_parts(&session, vec![completed_part(1, "etag-1")]).is_err());
		assert!(
			validate_completed_parts(
				&session,
				vec![completed_part(1, "etag-1"), completed_part(1, "etag-duplicate")]
			)
			.is_err()
		);
		assert!(
			validate_completed_parts(
				&session,
				vec![completed_part(1, "etag-1"), completed_part(2, " ")]
			)
			.is_err()
		);

		Ok(())
	}

	#[test]
	fn completed_upload_metadata_must_match_session_policy() {
		let session =
			upload_session_for_test(10, ObjectLifecycleConfig::S3_MIN_MULTIPART_PART_SIZE_BYTES);

		assert_eq!(
			completed_upload_metadata_error(
				&session,
				&StoredObjectMetadata {
					content_length: 10,
					content_type: "image/png".to_string(),
				}
			),
			None
		);
		assert!(
			completed_upload_metadata_error(
				&session,
				&StoredObjectMetadata {
					content_length: 11,
					content_type: "image/png".to_string(),
				}
			)
			.is_some()
		);
		assert!(
			completed_upload_metadata_error(
				&session,
				&StoredObjectMetadata {
					content_length: 10,
					content_type: "image/jpeg".to_string(),
				}
			)
			.is_some()
		);
	}

	fn storage_keys(
		prefix: &str,
		count: usize,
	) -> Vec<String> {
		(0 .. count).map(|index| format!("{prefix}-{index}")).collect()
	}

	fn completed_part(
		part_number: i32,
		e_tag: &str,
	) -> CompletedUploadPart {
		CompletedUploadPart {
			part_number,
			e_tag: e_tag.to_string(),
		}
	}

	fn upload_session_for_test(
		file_size_bytes: i64,
		part_size_bytes: i64,
	) -> ObjectUploadSession {
		ObjectUploadSession {
			object_id: 1,
			storage_key: "objects/test".to_string(),
			upload_id: "upload-id".to_string(),
			content_type: "image/png".to_string(),
			file_size_bytes,
			part_size_bytes,
			expires_at: Timestamp::now(),
		}
	}
}
