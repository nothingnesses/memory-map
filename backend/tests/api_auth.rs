use {
	anyhow::Context,
	aws_sdk_s3::primitives::ByteStream,
	axum::{
		Router,
		body::{
			Body,
			Bytes,
			to_bytes,
		},
		http::{
			HeaderMap,
			Request,
			StatusCode,
			header,
		},
	},
	backend::{
		Config,
		SharedState,
		app::{
			build_app,
			build_shared_state,
		},
		db::queries::{
			CLAIM_OBJECT_STORAGE_DELETIONS_QUERY,
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
		},
		email_worker::EmailOutboxConfig,
		migrations,
		object_lifecycle::{
			ObjectLifecycleConfig,
			ObjectLifecycleWorker,
		},
		storage::{
			CompletedUploadPart,
			MultipartUploadCompleteOutcome,
			StorageClient,
			StorageConfig,
		},
	},
	casbin::{
		CoreApi,
		DefaultModel,
		Enforcer,
		FileAdapter,
	},
	deadpool::managed::{
		Object,
		Pool,
	},
	deadpool_postgres::{
		Manager,
		Runtime,
	},
	serde_json::{
		Value,
		json,
	},
	std::{
		env,
		ops::DerefMut,
		path::Path,
		sync::Arc,
		time::{
			Duration,
			SystemTime,
			UNIX_EPOCH,
		},
	},
	tokio::{
		net::TcpStream,
		sync::RwLock,
		time::timeout,
	},
	tokio_postgres::NoTls,
	tower::ServiceExt,
};

const TEST_RESPONSE_BODY_LIMIT_BYTES: usize = 1024 * 1024;

struct TestApp {
	app: Router,
	state: Arc<SharedState<Manager, Object<Manager>>>,
}

struct TestResponse {
	status: StatusCode,
	headers: HeaderMap,
	body: Bytes,
}

struct DirectUploadResult {
	object: Value,
	object_id: String,
	completed_parts: Vec<Value>,
}

struct PreparedDirectUpload {
	object_id: String,
	completed_parts: Vec<Value>,
}

struct UploadSessionStorage {
	storage_key: String,
	upload_id: String,
}

struct DirectUploadRequest<'a> {
	object_name: &'a str,
	location: Option<Value>,
	made_on: Option<&'a str>,
	content_type: &'a str,
	body: &'a [u8],
}

impl<'a> DirectUploadRequest<'a> {
	fn svg(
		object_name: &'a str,
		latitude: f64,
		longitude: f64,
		body: &'a [u8],
	) -> Self {
		Self {
			object_name,
			location: Some(json!({
				"latitude": latitude,
				"longitude": longitude,
			})),
			made_on: Some("2026-05-31T12:00:00Z"),
			content_type: "image/svg+xml",
			body,
		}
	}

	fn svg_without_location(
		object_name: &'a str,
		body: &'a [u8],
	) -> Self {
		Self {
			object_name,
			location: None,
			made_on: Some("2026-05-31T12:00:00Z"),
			content_type: "image/svg+xml",
			body,
		}
	}

	fn with_location(
		mut self,
		location: Option<Value>,
	) -> Self {
		self.location = location;
		self
	}

	fn with_made_on(
		mut self,
		made_on: &'a str,
	) -> Self {
		self.made_on = Some(made_on);
		self
	}

	fn with_content_type(
		mut self,
		content_type: &'a str,
	) -> Self {
		self.content_type = content_type;
		self
	}
}

impl TestResponse {
	fn json(&self) -> anyhow::Result<Value> {
		serde_json::from_slice(&self.body).context("response body is not valid JSON")
	}
}

impl TestApp {
	async fn new() -> anyhow::Result<Option<Self>> {
		let cfg = test_config()?;

		if !postgres_is_reachable(&cfg).await? {
			return skip_or_fail("PostgreSQL is not reachable".to_string());
		}
		if !storage_endpoint_is_reachable(&cfg.storage.endpoint_url).await? {
			return skip_or_fail(format!(
				"storage endpoint is not reachable: {}",
				cfg.storage.endpoint_url
			));
		}

		let pool = cfg.pg.create_pool(Some(Runtime::Tokio1), NoTls)?;
		run_migrations(&pool).await?;

		let storage = StorageClient::from_config(&cfg)?;
		storage.ensure_bucket_exists().await?;

		let enforcer = test_enforcer().await?;
		let shared_state = build_shared_state(cfg, pool, storage, Arc::new(RwLock::new(enforcer)));
		let app = build_app(shared_state.clone());

		Ok(Some(Self {
			app,
			state: shared_state,
		}))
	}

	async fn request(
		&self,
		request: Request<Body>,
	) -> anyhow::Result<TestResponse> {
		let response = self
			.app
			.clone()
			.oneshot(request)
			.await
			.map_err(|err| anyhow::anyhow!("router request failed: {err}"))?;
		let (parts, body) = response.into_parts();
		let body = to_bytes(body, TEST_RESPONSE_BODY_LIMIT_BYTES).await?;

		Ok(TestResponse {
			status: parts.status,
			headers: parts.headers,
			body,
		})
	}

	async fn graphql(
		&self,
		query: &str,
		variables: Value,
		cookie: Option<&str>,
	) -> anyhow::Result<TestResponse> {
		let body = json!({
			"query": query,
			"variables": variables,
		});
		let mut request = Request::builder()
			.method("POST")
			.uri("/")
			.header(header::CONTENT_TYPE, "application/json");

		if let Some(cookie) = cookie {
			request = request.header(header::COOKIE, cookie);
		}

		self.request(request.body(Body::from(body.to_string()))?).await
	}

	async fn create_object_upload_session(
		&self,
		cookie: Option<&str>,
		upload: &DirectUploadRequest<'_>,
	) -> anyhow::Result<TestResponse> {
		self.graphql(
			"mutation CreateObjectUploadSession($input: CreateObjectUploadSessionInput!) {
				createObjectUploadSession(input: $input) {
					objectId
					partSizeBytes
					totalParts
				}
			}",
			json!({
				"input": {
					"name": upload.object_name,
					"contentType": upload.content_type,
					"fileSizeBytes": upload.body.len() as i64,
					"madeOn": upload.made_on,
					"location": upload.location,
					"publicity": "DEFAULT",
					"allowedUsers": [],
				},
			}),
			cookie,
		)
		.await
	}

	async fn direct_upload_object(
		&self,
		cookie: &str,
		upload: &DirectUploadRequest<'_>,
	) -> anyhow::Result<Value> {
		Ok(self.direct_upload_object_with_result(cookie, upload).await?.object)
	}

	async fn direct_upload_object_with_result(
		&self,
		cookie: &str,
		upload: &DirectUploadRequest<'_>,
	) -> anyhow::Result<DirectUploadResult> {
		let prepared = self.prepare_direct_upload(cookie, upload).await?;
		let complete = self
			.complete_object_upload(cookie, &prepared.object_id, prepared.completed_parts.clone())
			.await?;
		assert_eq!(complete.status, StatusCode::OK);
		let complete = complete.json()?;
		assert_graphql_success(&complete)?;
		let object = json_path(&complete, &["data", "completeObjectUpload"])?.clone();
		Ok(DirectUploadResult {
			object,
			object_id: prepared.object_id,
			completed_parts: prepared.completed_parts,
		})
	}

	async fn prepare_direct_upload(
		&self,
		cookie: &str,
		upload: &DirectUploadRequest<'_>,
	) -> anyhow::Result<PreparedDirectUpload> {
		let create = self.create_object_upload_session(Some(cookie), upload).await?;
		assert_eq!(create.status, StatusCode::OK);
		let create = create.json()?;
		assert_graphql_success(&create)?;
		let session = json_path(&create, &["data", "createObjectUploadSession"])?;
		let object_id = json_path(session, &["objectId"])?
			.as_str()
			.context("createObjectUploadSession.objectId is not a string")?
			.to_string();
		let part_size_bytes = json_path(session, &["partSizeBytes"])?
			.as_i64()
			.context("createObjectUploadSession.partSizeBytes is not an integer")?;
		let total_parts = json_path(session, &["totalParts"])?
			.as_i64()
			.context("createObjectUploadSession.totalParts is not an integer")?;

		let mut completed_parts = Vec::with_capacity(total_parts as usize);
		let http = reqwest::Client::new();
		for part_number in 1 ..= total_parts {
			let presign = self
				.graphql(
					"mutation PresignObjectUploadParts($objectId: ID!, $partNumbers: [Int!]!) {
						presignObjectUploadParts(objectId: $objectId, partNumbers: $partNumbers) {
							partNumber
							url
							method
							headers { name value }
							expectedContentLength
						}
					}",
					json!({
						"objectId": object_id,
						"partNumbers": [part_number],
					}),
					Some(cookie),
				)
				.await?;
			assert_eq!(presign.status, StatusCode::OK);
			let presign = presign.json()?;
			assert_graphql_success(&presign)?;
			let parts = json_path(&presign, &["data", "presignObjectUploadParts"])?
				.as_array()
				.context("presignObjectUploadParts response is not an array")?;
			let part = parts.first().context("presignObjectUploadParts response is empty")?;
			let expected_content_length = json_path(part, &["expectedContentLength"])?
				.as_i64()
				.context("expectedContentLength is not an integer")?;
			let start = ((part_number - 1) * part_size_bytes) as usize;
			let end = start + expected_content_length as usize;
			let chunk = upload
				.body
				.get(start .. end)
				.with_context(|| format!("part {part_number} byte range was out of bounds"))?;
			assert_eq!(chunk.len(), expected_content_length as usize);

			let method = json_path(part, &["method"])?
				.as_str()
				.context("presigned part method is not a string")?
				.parse::<reqwest::Method>()?;
			let url = json_path(part, &["url"])?
				.as_str()
				.context("presigned part URL is not a string")?;
			let mut request = http.request(method, url).body(chunk.to_vec());
			for header in json_path(part, &["headers"])?
				.as_array()
				.context("presigned part headers is not an array")?
			{
				let name = json_path(header, &["name"])?
					.as_str()
					.context("presigned header name is not a string")?
					.parse::<reqwest::header::HeaderName>()?;
				let value = json_path(header, &["value"])?
					.as_str()
					.context("presigned header value is not a string")?
					.parse::<reqwest::header::HeaderValue>()?;
				request = request.header(name, value);
			}
			let response = request.send().await?;
			let status = response.status();
			assert!(status.is_success(), "part upload failed with {status}");
			let e_tag = response
				.headers()
				.get(reqwest::header::ETAG)
				.with_context(|| format!("part {part_number} upload did not return ETag"))?
				.to_str()?
				.to_string();
			completed_parts.push(json!({
				"partNumber": part_number,
				"eTag": e_tag,
			}));
		}

		Ok(PreparedDirectUpload {
			object_id,
			completed_parts,
		})
	}

	async fn complete_object_upload(
		&self,
		cookie: &str,
		object_id: &str,
		completed_parts: Vec<Value>,
	) -> anyhow::Result<TestResponse> {
		self.graphql(
			"mutation CompleteObjectUpload($objectId: ID!, $parts: [CompletedObjectUploadPartInput!]!) {
				completeObjectUpload(objectId: $objectId, parts: $parts) {
					id
					name
					location { latitude longitude }
					contentType
				}
			}",
			json!({
				"objectId": object_id,
				"parts": completed_parts,
			}),
			Some(cookie),
		)
		.await
	}

	async fn object_count(
		&self,
		object_name: &str,
	) -> anyhow::Result<i64> {
		let client = self.state.pool.get().await?;
		let count = client
			.query_one("SELECT COUNT(*) FROM objects WHERE name = $1", &[&object_name])
			.await?
			.get(0);
		Ok(count)
	}

	async fn object_storage_key(
		&self,
		object_name: &str,
	) -> anyhow::Result<String> {
		let client = self.state.pool.get().await?;
		let storage_key = client
			.query_one("SELECT storage_key FROM objects WHERE name = $1", &[&object_name])
			.await?
			.get(0);
		Ok(storage_key)
	}

	async fn object_content_type(
		&self,
		object_name: &str,
	) -> anyhow::Result<String> {
		let client = self.state.pool.get().await?;
		let content_type = client
			.query_one("SELECT content_type FROM objects WHERE name = $1", &[&object_name])
			.await?
			.get(0);
		Ok(content_type)
	}

	async fn password_reset_token_count(
		&self,
		email: &str,
	) -> anyhow::Result<i64> {
		let client = self.state.pool.get().await?;
		let count = client
			.query_one(
				"SELECT COUNT(*)
				FROM password_reset_tokens token
				JOIN users user_account ON user_account.id = token.user_id
				WHERE user_account.email = $1
					AND token.expires_at > now()",
				&[&email],
			)
			.await?
			.get(0);
		Ok(count)
	}

	async fn password_reset_email_outbox_count(
		&self,
		email: &str,
	) -> anyhow::Result<i64> {
		let client = self.state.pool.get().await?;
		let count = client
			.query_one(
				"SELECT COUNT(*)
				FROM email_outbox
				WHERE kind = 'password_reset'
					AND payload->>'email' = $1",
				&[&email],
			)
			.await?
			.get(0);
		Ok(count)
	}

	async fn upload_session_storage(
		&self,
		object_id: &str,
	) -> anyhow::Result<UploadSessionStorage> {
		let object_id = object_id.parse::<i64>()?;
		let client = self.state.pool.get().await?;
		let row = client
			.query_one(
				"SELECT storage_key, upload_id FROM object_upload_sessions WHERE object_id = $1",
				&[&object_id],
			)
			.await?;
		Ok(UploadSessionStorage {
			storage_key: row.get("storage_key"),
			upload_id: row.get("upload_id"),
		})
	}

	async fn expire_upload_session(
		&self,
		object_id: &str,
	) -> anyhow::Result<()> {
		let object_id = object_id.parse::<i64>()?;
		let client = self.state.pool.get().await?;
		client
			.execute(
				"UPDATE object_upload_sessions
				SET created_at = now() - interval '2 seconds',
					expires_at = now() - interval '1 second',
					cleanup_next_attempt_at = now() - interval '1 second'
				WHERE object_id = $1",
				&[&object_id],
			)
			.await?;
		Ok(())
	}

	async fn upload_session_count(
		&self,
		object_name: &str,
	) -> anyhow::Result<i64> {
		let client = self.state.pool.get().await?;
		let count = client
			.query_one(
				"SELECT COUNT(*)
				FROM object_upload_sessions session
				JOIN objects object ON object.id = session.object_id
				WHERE object.name = $1",
				&[&object_name],
			)
			.await?
			.get(0);
		Ok(count)
	}

	async fn storage_deletion_outbox_count_for_key(
		&self,
		storage_key: &str,
	) -> anyhow::Result<i64> {
		let client = self.state.pool.get().await?;
		let count = client
			.query_one(
				"SELECT COUNT(*) FROM object_storage_deletions WHERE storage_key = $1",
				&[&storage_key],
			)
			.await?
			.get(0);
		Ok(count)
	}

	async fn run_object_lifecycle_maintenance(
		&self,
		config: ObjectLifecycleConfig,
	) -> anyhow::Result<()> {
		ObjectLifecycleWorker::new(self.state.pool.clone(), self.state.storage.clone(), config)
			.run_once()
			.await
			.map_err(Into::into)
	}
}

struct TestUser {
	email: String,
	cookie: String,
}

async fn register_and_login(app: &TestApp) -> anyhow::Result<TestUser> {
	let email = format!("api-auth-{}@example.test", unique_suffix()?);
	let password = "memory-map-test-password";

	let register = app
		.graphql(
			"mutation Register($email: String!, $password: String!) {
				register(email: $email, password: $password) { id email role }
			}",
			json!({
				"email": email,
				"password": password,
			}),
			None,
		)
		.await?;
	assert_eq!(register.status, StatusCode::OK);
	assert_graphql_success(&register.json()?)?;

	let login = app
		.graphql(
			"mutation Login($email: String!, $password: String!) {
				login(email: $email, password: $password) { id email role }
			}",
			json!({
				"email": email,
				"password": password,
			}),
			None,
		)
		.await?;
	assert_eq!(login.status, StatusCode::OK);
	assert_graphql_success(&login.json()?)?;
	let cookie = auth_cookie(&login.headers)?;

	Ok(TestUser {
		email,
		cookie,
	})
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn unauthenticated_upload_is_rejected() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let object_name = format!("unauthenticated-upload-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg(
		&object_name,
		12.5,
		-45.25,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	);

	let response = app.create_object_upload_session(None, &upload).await?;

	assert_eq!(response.status, StatusCode::OK);
	assert_graphql_error_contains(&response.json()?, "Unauthorized")?;
	assert_eq!(app.object_count(&object_name).await?, 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn login_cookie_authenticates_graphql_requests() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;

	let me = app.graphql("query Me { me { email } }", json!({}), Some(&user.cookie)).await?;
	assert_eq!(me.status, StatusCode::OK);
	let me = me.json()?;
	assert_graphql_success(&me)?;
	assert_eq!(json_path(&me, &["data", "me", "email"])?.as_str(), Some(user.email.as_str()));

	let anonymous_me = app.graphql("query Me { me { email } }", json!({}), None).await?;
	assert_eq!(anonymous_me.status, StatusCode::OK);
	let anonymous_me = anonymous_me.json()?;
	assert_graphql_success(&anonymous_me)?;
	assert!(json_path(&anonymous_me, &["data", "me"])?.is_null());

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn authenticated_query_auth_db_error_returns_500_without_cache_write() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	assert_eq!(app.state.graphql_response_cache.entry_count(), 0);

	app.state.pool.close();
	let response = app
		.graphql("query Config { config { enableRegistration } }", json!({}), Some(&user.cookie))
		.await?;

	assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
	assert_eq!(app.state.graphql_response_cache.entry_count(), 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn password_reset_request_enqueues_one_email_inside_rate_limit() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;

	for _ in 0 .. 2 {
		let response = app
			.graphql(
				"mutation RequestPasswordReset($email: String!) {
					requestPasswordReset(email: $email)
				}",
				json!({
					"email": user.email.clone(),
				}),
				None,
			)
			.await?;
		assert_eq!(response.status, StatusCode::OK);
		let response = response.json()?;
		assert_graphql_success(&response)?;
		assert_eq!(json_path(&response, &["data", "requestPasswordReset"])?.as_bool(), Some(true));
	}

	assert_eq!(app.password_reset_token_count(&user.email).await?, 1);
	assert_eq!(app.password_reset_email_outbox_count(&user.email).await?, 1);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn authenticated_upload_preserves_content_type_and_delete_cleans_up() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("authenticated-upload-{}.svg", unique_suffix()?);
	let body = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"1\" height=\"1\" /></svg>";
	let upload = DirectUploadRequest::svg(&object_name, 12.5, -45.25, body);

	let upload_result = app.direct_upload_object_with_result(&user.cookie, &upload).await?;
	let uploaded_object = upload_result.object;
	let object_id = json_path(&uploaded_object, &["id"])?
		.as_str()
		.context("upload response is missing object id")?;
	assert_eq!(json_path(&uploaded_object, &["name"])?.as_str(), Some(object_name.as_str()));

	let retry_complete = app
		.complete_object_upload(
			&user.cookie,
			&upload_result.object_id,
			upload_result.completed_parts.clone(),
		)
		.await?;
	assert_eq!(retry_complete.status, StatusCode::OK);
	let retry_complete = retry_complete.json()?;
	assert_graphql_success(&retry_complete)?;
	assert_eq!(
		json_path(&retry_complete, &["data", "completeObjectUpload", "id"])?.as_str(),
		Some(object_id)
	);

	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.object_content_type(&object_name).await?, "image/svg+xml");
	let storage_key = app.object_storage_key(&object_name).await?;
	assert_eq!(app.state.storage.object_content_type(&storage_key).await?, "image/svg+xml");

	let protected_query = app
		.graphql(
			"query ObjectByName($name: String!) {
				s3ObjectByName(name: $name) { id name }
			}",
			json!({
				"name": object_name,
			}),
			None,
		)
		.await?;
	assert_eq!(protected_query.status, StatusCode::OK);
	assert_graphql_error_contains(&protected_query.json()?, "Unauthorized")?;

	let visible_objects = app
		.graphql(
			"query Objects { s3Objects { id name contentType } }",
			json!({}),
			Some(&user.cookie),
		)
		.await?;
	assert_eq!(visible_objects.status, StatusCode::OK);
	let visible_objects = visible_objects.json()?;
	assert_graphql_success(&visible_objects)?;
	let object = json_path(&visible_objects, &["data", "s3Objects"])?
		.as_array()
		.context("s3Objects response is not an array")?
		.iter()
		.find(|object| {
			json_path(object, &["name"])
				.and_then(|name| name.as_str().context("object name is not a string"))
				.is_ok_and(|name| name == object_name)
		})
		.context("uploaded object is missing from s3Objects")?;
	assert_eq!(json_path(object, &["contentType"])?.as_str(), Some("image/svg+xml"));

	let delete = app
		.graphql(
			"mutation Delete($ids: [ID!]!) {
				deleteS3Objects(ids: $ids) { id name }
			}",
			json!({
				"ids": [object_id],
			}),
			Some(&user.cookie),
		)
		.await?;
	assert_eq!(delete.status, StatusCode::OK);
	assert_graphql_success(&delete.json()?)?;
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.storage_deletion_outbox_count_for_key(&storage_key).await?, 1);

	let visible_objects = app
		.graphql(
			"query Objects { s3Objects { id name contentType } }",
			json!({}),
			Some(&user.cookie),
		)
		.await?;
	assert_eq!(visible_objects.status, StatusCode::OK);
	let visible_objects = visible_objects.json()?;
	assert_graphql_success(&visible_objects)?;
	let still_visible = json_path(&visible_objects, &["data", "s3Objects"])?
		.as_array()
		.context("s3Objects response is not an array")?
		.iter()
		.any(|object| {
			json_path(object, &["name"])
				.and_then(|name| name.as_str().context("object name is not a string"))
				.is_ok_and(|name| name == object_name)
		});
	assert!(!still_visible);

	app.run_object_lifecycle_maintenance(ObjectLifecycleConfig::default()).await?;
	assert_eq!(app.object_count(&object_name).await?, 0);
	assert!(app.state.storage.object_content_type(&storage_key).await.is_err());
	assert_eq!(app.storage_deletion_outbox_count_for_key(&storage_key).await?, 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn direct_upload_accepts_multiple_parts() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("multipart-upload-{}.svg", unique_suffix()?);
	let part_size = app.state.config.object_lifecycle.upload_part_size_bytes as usize;
	let body = vec![b'x'; part_size + 17];
	let upload = DirectUploadRequest::svg(&object_name, 12.5, -45.25, &body);

	let uploaded_object = app.direct_upload_object(&user.cookie, &upload).await?;
	assert_eq!(json_path(&uploaded_object, &["name"])?.as_str(), Some(object_name.as_str()));
	assert_eq!(app.object_count(&object_name).await?, 1);
	let storage_key = app.object_storage_key(&object_name).await?;
	let metadata = app.state.storage.head_object(&storage_key).await?;
	assert_eq!(metadata.content_length, body.len() as i64);
	assert_eq!(metadata.content_type, "image/svg+xml");

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn invalid_upload_coordinates_do_not_leave_side_effects() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;

	for (label, latitude, longitude, expected_error) in [
		("latitude", 90.1, -45.25, "not a valid latitude value"),
		("longitude", 12.5, -180.1, "not a valid longitude value"),
	] {
		let object_name = format!("invalid-{label}-upload-{}.svg", unique_suffix()?);
		let upload = DirectUploadRequest::svg(
			&object_name,
			latitude,
			longitude,
			b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
		);

		let response = app.create_object_upload_session(Some(&user.cookie), &upload).await?;

		assert_eq!(response.status, StatusCode::OK);
		assert_graphql_error_contains(&response.json()?, expected_error)?;
		assert_eq!(app.object_count(&object_name).await?, 0);
	}

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn upload_without_coordinates_stores_no_location() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("no-location-upload-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg_without_location(
		&object_name,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	);

	let uploaded_object = app.direct_upload_object(&user.cookie, &upload).await?;
	assert!(json_path(&uploaded_object, &["location"])?.is_null());
	assert_eq!(app.object_count(&object_name).await?, 1);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn partial_upload_coordinates_do_not_leave_side_effects() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;

	for (label, location, expected_error) in [
		("latitude-only", json!({ "latitude": 12.5 }), "longitude"),
		("longitude-only", json!({ "longitude": -45.25 }), "latitude"),
	] {
		let object_name = format!("partial-{label}-upload-{}.svg", unique_suffix()?);
		let upload = DirectUploadRequest::svg_without_location(
			&object_name,
			b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
		)
		.with_location(Some(location));

		let response = app.create_object_upload_session(Some(&user.cookie), &upload).await?;

		assert_eq!(response.status, StatusCode::OK);
		assert_graphql_error_contains(&response.json()?, expected_error)?;
		assert_eq!(app.object_count(&object_name).await?, 0);
	}

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn invalid_upload_timestamp_does_not_leave_side_effects() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("invalid-timestamp-upload-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg(
		&object_name,
		12.5,
		-45.25,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	)
	.with_made_on("not-a-timestamp");

	let response = app.create_object_upload_session(Some(&user.cookie), &upload).await?;

	assert_eq!(response.status, StatusCode::OK);
	assert_graphql_error_contains(&response.json()?, "Invalid timestamp format")?;
	assert_eq!(app.object_count(&object_name).await?, 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn duplicate_upload_name_does_not_overwrite_existing_object() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("duplicate-upload-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg(
		&object_name,
		12.5,
		-45.25,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	);

	app.direct_upload_object(&user.cookie, &upload).await?;
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.object_content_type(&object_name).await?, "image/svg+xml");
	let storage_key = app.object_storage_key(&object_name).await?;
	assert_eq!(app.state.storage.object_content_type(&storage_key).await?, "image/svg+xml");

	let duplicate = DirectUploadRequest::svg(&object_name, 12.5, -45.25, b"not really a jpeg")
		.with_content_type("image/jpeg");
	let duplicate_upload = app.create_object_upload_session(Some(&user.cookie), &duplicate).await?;

	assert_eq!(duplicate_upload.status, StatusCode::OK);
	assert_graphql_error_contains(&duplicate_upload.json()?, "already exists")?;
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.object_storage_key(&object_name).await?, storage_key);
	assert_eq!(app.object_content_type(&object_name).await?, "image/svg+xml");
	assert_eq!(app.state.storage.object_content_type(&storage_key).await?, "image/svg+xml");

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn stale_pending_upload_cleanup_removes_blob_metadata_and_releases_name() -> anyhow::Result<()>
{
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("stale-pending-upload-{}.svg", unique_suffix()?);
	let storage_key = format!("objects/stale-pending-upload-{}", unique_suffix()?);
	let body = b"<svg xmlns=\"http://www.w3.org/2000/svg\" />";

	app.state
		.storage
		.upload_object(&storage_key, ByteStream::from_static(body), "image/svg+xml")
		.await?;

	let client = app.state.pool.get().await?;
	let user_id: i64 =
		client.query_one("SELECT id FROM users WHERE email = $1", &[&user.email]).await?.get(0);
	client
		.execute(
			"INSERT INTO objects (
				name,
				storage_key,
				content_type,
				storage_state,
				storage_state_updated_at,
				user_id,
				publicity
			)
			VALUES ($1, $2, 'image/svg+xml', 'pending_upload', now() - interval '2 hours', $3, 'default')",
			&[&object_name, &storage_key, &user_id],
		)
		.await?;
	drop(client);

	let lifecycle_config = ObjectLifecycleConfig {
		pending_upload_timeout_seconds: 1,
		storage_deletion_retry_seconds: 1,
		storage_deletion_lease_seconds: 30,
		storage_deletion_worker_interval_seconds: 1,
		storage_deletion_batch_size: 1000,
		storage_deletion_max_attempts: 10,
		..ObjectLifecycleConfig::default()
	}
	.validated()?;
	app.run_object_lifecycle_maintenance(lifecycle_config).await?;

	assert_eq!(app.object_count(&object_name).await?, 0);
	assert!(app.state.storage.object_content_type(&storage_key).await.is_err());

	let upload = DirectUploadRequest::svg(&object_name, 12.5, -45.25, body);
	app.direct_upload_object(&user.cookie, &upload).await?;
	assert_eq!(app.object_count(&object_name).await?, 1);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn expired_incomplete_upload_session_cleanup_removes_metadata() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("expired-upload-session-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg(
		&object_name,
		12.5,
		-45.25,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	);

	let create = app.create_object_upload_session(Some(&user.cookie), &upload).await?;
	assert_eq!(create.status, StatusCode::OK);
	let create = create.json()?;
	assert_graphql_success(&create)?;
	let object_id = json_path(&create, &["data", "createObjectUploadSession", "objectId"])?
		.as_str()
		.context("createObjectUploadSession.objectId is not a string")?
		.to_string();
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.upload_session_count(&object_name).await?, 1);

	app.expire_upload_session(&object_id).await?;

	let lifecycle_config = ObjectLifecycleConfig {
		upload_session_cleanup_retry_seconds: 1,
		upload_session_cleanup_lease_seconds: 30,
		upload_session_cleanup_max_attempts: 10,
		upload_session_cleanup_batch_size: 100,
		..ObjectLifecycleConfig::default()
	}
	.validated()?;
	app.run_object_lifecycle_maintenance(lifecycle_config).await?;

	assert_eq!(app.object_count(&object_name).await?, 0);
	assert_eq!(app.upload_session_count(&object_name).await?, 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn expired_completed_upload_session_cleanup_removes_completed_orphan() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("expired-completed-upload-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg(
		&object_name,
		12.5,
		-45.25,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	);
	let prepared = app.prepare_direct_upload(&user.cookie, &upload).await?;
	let session = app.upload_session_storage(&prepared.object_id).await?;
	let completed_parts = completed_upload_parts(&prepared.completed_parts)?;

	assert_eq!(
		app.state
			.storage
			.complete_multipart_upload(&session.storage_key, &session.upload_id, &completed_parts)
			.await?,
		MultipartUploadCompleteOutcome::Completed
	);
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.upload_session_count(&object_name).await?, 1);

	app.expire_upload_session(&prepared.object_id).await?;
	app.run_object_lifecycle_maintenance(ObjectLifecycleConfig::default()).await?;

	assert_eq!(app.object_count(&object_name).await?, 0);
	assert_eq!(app.upload_session_count(&object_name).await?, 0);
	assert!(app.state.storage.object_content_type(&session.storage_key).await.is_err());
	assert_eq!(app.storage_deletion_outbox_count_for_key(&session.storage_key).await?, 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn completed_upload_metadata_mismatch_queues_completed_object_cleanup() -> anyhow::Result<()>
{
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let user = register_and_login(&app).await?;
	let object_name = format!("metadata-mismatch-upload-{}.svg", unique_suffix()?);
	let upload = DirectUploadRequest::svg(
		&object_name,
		12.5,
		-45.25,
		b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
	);

	let create = app.create_object_upload_session(Some(&user.cookie), &upload).await?;
	assert_eq!(create.status, StatusCode::OK);
	let create = create.json()?;
	assert_graphql_success(&create)?;
	let object_id = json_path(&create, &["data", "createObjectUploadSession", "objectId"])?
		.as_str()
		.context("createObjectUploadSession.objectId is not a string")?
		.to_string();
	let session = app.upload_session_storage(&object_id).await?;
	app.state.storage.abort_multipart_upload(&session.storage_key, &session.upload_id).await?;
	app.state
		.storage
		.upload_object(&session.storage_key, ByteStream::from_static(b"wrong-size"), "image/jpeg")
		.await?;

	let complete = app
		.complete_object_upload(
			&user.cookie,
			&object_id,
			vec![json!({
				"partNumber": 1,
				"eTag": "orphan-etag",
			})],
		)
		.await?;
	assert_eq!(complete.status, StatusCode::OK);
	assert_graphql_error_contains(
		&complete.json()?,
		"Completed upload size did not match declared file size",
	)?;
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.upload_session_count(&object_name).await?, 0);
	assert_eq!(app.storage_deletion_outbox_count_for_key(&session.storage_key).await?, 1);

	app.run_object_lifecycle_maintenance(ObjectLifecycleConfig::default()).await?;

	assert_eq!(app.object_count(&object_name).await?, 0);
	assert!(app.state.storage.object_content_type(&session.storage_key).await.is_err());
	assert_eq!(app.storage_deletion_outbox_count_for_key(&session.storage_key).await?, 0);

	Ok(())
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL and RustFS service graph"]
async fn object_storage_deletion_claims_respect_lease_and_retry() -> anyhow::Result<()> {
	let Some(app) = TestApp::new().await? else {
		return Ok(());
	};
	let first_key = format!("objects/claim-lease-first-{}", unique_suffix()?);
	let second_key = format!("objects/claim-lease-second-{}", unique_suffix()?);

	let mut client = app.state.pool.get().await?;
	let transaction = client.transaction().await?;
	transaction
		.execute(
			"UPDATE object_storage_deletions
			SET next_attempt_at = now() + interval '1 hour'",
			&[],
		)
		.await?;
	transaction
		.execute(
			"INSERT INTO object_storage_deletions (storage_key) VALUES ($1), ($2)",
			&[&first_key, &second_key],
		)
		.await?;

	let first_claim = claim_keys(&transaction, 1, 600, 10).await?;
	assert_eq!(first_claim.len(), 1);
	let first_claimed_key =
		first_claim.into_iter().next().context("first object storage deletion claim was empty")?;
	let remaining_key =
		if first_claimed_key == first_key { second_key.clone() } else { first_key.clone() };

	assert_eq!(claim_keys(&transaction, 10, 600, 10).await?, vec![remaining_key.clone()]);
	assert!(claim_keys(&transaction, 10, 600, 10).await?.is_empty());

	transaction
		.execute(
			MARK_OBJECT_STORAGE_DELETIONS_FAILED_QUERY,
			&[&vec![first_claimed_key.clone()], &"simulated storage failure", &60_i64],
		)
		.await?;
	assert!(claim_keys(&transaction, 10, 600, 10).await?.is_empty());

	transaction
		.execute(
			"UPDATE object_storage_deletions
			SET next_attempt_at = now() - interval '1 second'
			WHERE storage_key = $1",
			&[&first_claimed_key],
		)
		.await?;
	assert_eq!(claim_keys(&transaction, 10, 600, 10).await?, vec![first_claimed_key.clone()]);

	let first_attempts: i32 = transaction
		.query_one(
			"SELECT attempts FROM object_storage_deletions WHERE storage_key = $1",
			&[&first_claimed_key],
		)
		.await?
		.get(0);
	assert_eq!(first_attempts, 2);

	transaction.rollback().await?;

	Ok(())
}

async fn claim_keys(
	client: &impl deadpool_postgres::GenericClient,
	limit: i64,
	lease_seconds: i64,
	max_attempts: i32,
) -> Result<Vec<String>, tokio_postgres::Error> {
	client
		.query(CLAIM_OBJECT_STORAGE_DELETIONS_QUERY, &[&limit, &lease_seconds, &max_attempts])
		.await
		.map(|rows| {
			rows.into_iter().map(|row| row.get::<_, String>("storage_key")).collect::<Vec<_>>()
		})
}

fn completed_upload_parts(parts: &[Value]) -> anyhow::Result<Vec<CompletedUploadPart>> {
	parts
		.iter()
		.map(|part| {
			let part_number = json_path(part, &["partNumber"])?
				.as_i64()
				.context("completed part number is not an integer")
				.and_then(|part_number| {
					i32::try_from(part_number).context("completed part number exceeds i32 range")
				})?;
			let e_tag = json_path(part, &["eTag"])?
				.as_str()
				.context("completed part ETag is not a string")?
				.to_string();
			Ok(CompletedUploadPart {
				part_number,
				e_tag,
			})
		})
		.collect()
}

fn test_config() -> anyhow::Result<Config> {
	let pg = deadpool_postgres::Config {
		dbname: Some(env_or_default("MEMORY_MAP__PG__DBNAME", "db")),
		host: Some(env_or_default("MEMORY_MAP__PG__HOST", "127.0.0.1")),
		port: Some(env_or_default("MEMORY_MAP__PG__PORT", "5432").parse()?),
		..Default::default()
	};

	let frontend_url = env_or_default("MEMORY_MAP__FRONTEND__URL", "http://127.0.0.1:3000");

	let config = Config {
		pg,
		server: backend::ServerConfig {
			host: "127.0.0.1".to_string(),
			port: 8000,
		},
		smtp: backend::SmtpConfig {
			host: "smtp.example.test".to_string(),
			user: "memory-map-test".to_string(),
			pass: "memory-map-test-password".to_string(),
			from: "noreply@example.test".to_string(),
		},
		auth: backend::AuthConfig {
			cookie_secret: env_or_default(
				"MEMORY_MAP__AUTH__COOKIE_SECRET",
				"memory-map-local-test-cookie-secret-at-least-64-bytes-long-0001-extra",
			),
			enable_registration: true,
			cookie_secure: None,
		},
		frontend: backend::FrontendConfig {
			url: frontend_url.clone(),
		},
		cors: backend::CorsConfig {
			allowed_origins: env_or_default("MEMORY_MAP__CORS__ALLOWED_ORIGINS", &frontend_url),
		},
		storage: StorageConfig {
			endpoint_url: env_or_default(
				"MEMORY_MAP__STORAGE__ENDPOINT_URL",
				"http://127.0.0.1:9000/",
			),
			public_endpoint_url: std::env::var("MEMORY_MAP__STORAGE__PUBLIC_ENDPOINT_URL").ok(),
			access_key: env_or_default("MEMORY_MAP__STORAGE__ACCESS_KEY", "memorymapdev"),
			secret_key: env_or_default("MEMORY_MAP__STORAGE__SECRET_KEY", "memorymapdevsecret"),
			bucket_name: env_or_default("MEMORY_MAP__STORAGE__BUCKET_NAME", "memory-map"),
			region: env_or_default("MEMORY_MAP__STORAGE__REGION", &StorageConfig::default_region()),
			force_path_style: parse_bool_env(
				"MEMORY_MAP__STORAGE__FORCE_PATH_STYLE",
				StorageConfig::default_force_path_style(),
			)?,
			presigned_url_ttl_seconds: env_or_default(
				"MEMORY_MAP__STORAGE__PRESIGNED_URL_TTL_SECONDS",
				&StorageConfig::default_presigned_url_ttl_seconds().to_string(),
			)
			.parse()?,
		},
		object_lifecycle: ObjectLifecycleConfig::default(),
		email_outbox: EmailOutboxConfig::default(),
	};
	config.validated()
}

async fn run_migrations(pool: &Pool<Manager>) -> anyhow::Result<()> {
	let mut postgresql_connection = pool.get().await?;
	let postgresql_client = postgresql_connection.deref_mut().deref_mut();
	migrations::runner().run_async(postgresql_client).await?;
	Ok(())
}

async fn test_enforcer() -> anyhow::Result<Enforcer> {
	let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
	let model = DefaultModel::from_file(manifest_dir.join("authz_model.conf")).await?;
	let policy = FileAdapter::new(manifest_dir.join("authz_policy.csv"));
	Ok(Enforcer::new(model, policy).await?)
}

async fn postgres_is_reachable(config: &Config) -> anyhow::Result<bool> {
	let host = config.pg.host.as_deref().unwrap_or("127.0.0.1");
	let port = config.pg.port.unwrap_or(5432);
	Ok(matches!(timeout(Duration::from_secs(2), TcpStream::connect((host, port))).await, Ok(Ok(_))))
}

async fn storage_endpoint_is_reachable(endpoint_url: &str) -> anyhow::Result<bool> {
	let url = reqwest::Url::parse(endpoint_url)?;
	let host =
		url.host_str().ok_or_else(|| anyhow::anyhow!("S3 endpoint URL is missing a host"))?;
	let port = url
		.port_or_known_default()
		.ok_or_else(|| anyhow::anyhow!("S3 endpoint URL is missing a port"))?;
	Ok(matches!(timeout(Duration::from_secs(2), TcpStream::connect((host, port))).await, Ok(Ok(_))))
}

fn auth_cookie(headers: &HeaderMap) -> anyhow::Result<String> {
	let set_cookie = headers
		.get_all(header::SET_COOKIE)
		.iter()
		.find_map(|value| value.to_str().ok().filter(|value| value.starts_with("auth_token=")))
		.context("login response did not set auth_token cookie")?;
	assert!(set_cookie.contains("HttpOnly"));
	assert!(set_cookie.contains("SameSite=Lax"));
	assert!(set_cookie.contains("Path=/"));
	assert!(!set_cookie.contains("Secure"));
	Ok(set_cookie.split(';').next().context("Set-Cookie header is empty")?.to_string())
}

fn assert_graphql_success(value: &Value) -> anyhow::Result<()> {
	if let Some(errors) = value.get("errors") {
		anyhow::bail!("GraphQL response contained errors: {errors}");
	}
	Ok(())
}

fn json_path<'a>(
	value: &'a Value,
	path: &[&str],
) -> anyhow::Result<&'a Value> {
	let mut current = value;
	for key in path {
		current =
			current.get(*key).with_context(|| format!("JSON response is missing field {key}"))?;
	}
	Ok(current)
}

fn assert_graphql_error_contains(
	value: &Value,
	expected: &str,
) -> anyhow::Result<()> {
	let errors = value
		.get("errors")
		.and_then(Value::as_array)
		.context("GraphQL response did not contain errors")?;
	let has_expected_error = errors.iter().any(|error| {
		error
			.get("message")
			.and_then(Value::as_str)
			.is_some_and(|message| message.contains(expected))
	});

	if !has_expected_error {
		anyhow::bail!("GraphQL errors did not contain {expected:?}: {errors:?}");
	}

	Ok(())
}

fn skip_or_fail(message: String) -> anyhow::Result<Option<TestApp>> {
	if integration_service_required() {
		anyhow::bail!("{message}");
	}

	eprintln!("skipping backend integration test: {message}");
	Ok(None)
}

fn integration_service_required() -> bool {
	env::var("BACKEND_INTEGRATION_REQUIRE_SERVICE")
		.map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
		.unwrap_or(false)
}

fn unique_suffix() -> anyhow::Result<String> {
	use std::sync::atomic::{
		AtomicU64,
		Ordering,
	};
	// Tests in this file currently run with --test-threads=1, so the nanosecond
	// timestamp alone would suffice today. The atomic counter makes the suffix
	// robust against accidental parallelism (or two suffixes constructed in the
	// same nanosecond on a fast machine) without requiring the test runner config.
	static COUNTER: AtomicU64 = AtomicU64::new(0);
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
	let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
	Ok(format!("{}-{counter}", now.as_nanos()))
}

fn env_or_default(
	name: &str,
	default: &str,
) -> String {
	env::var(name).unwrap_or_else(|_| default.to_string())
}

fn parse_bool_env(
	name: &str,
	default: bool,
) -> anyhow::Result<bool> {
	let value = env_or_default(name, if default { "true" } else { "false" });
	match value.to_ascii_lowercase().as_str() {
		"1" | "true" | "yes" | "on" => Ok(true),
		"0" | "false" | "no" | "off" => Ok(false),
		_ => anyhow::bail!("{name} must be a boolean"),
	}
}
