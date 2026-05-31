use {
	anyhow::Context,
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
		constants::BODY_MAX_SIZE_LIMIT_BYTES,
		migrations,
		storage::StorageClient,
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

struct TestApp {
	app: Router,
	state: Arc<SharedState<Manager, Object<Manager>>>,
}

struct TestResponse {
	status: StatusCode,
	headers: HeaderMap,
	body: Bytes,
}

impl TestResponse {
	fn text(&self) -> anyhow::Result<&str> {
		std::str::from_utf8(&self.body).context("response body is not valid UTF-8")
	}

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
		if !storage_endpoint_is_reachable(&cfg.s3_endpoint_url).await? {
			return skip_or_fail(format!(
				"storage endpoint is not reachable: {}",
				cfg.s3_endpoint_url
			));
		}

		let pool = cfg.pg.create_pool(Some(Runtime::Tokio1), NoTls)?;
		run_migrations(&pool).await?;

		let storage = StorageClient::from_config(&cfg)?;
		storage.ensure_bucket_ready().await?;

		let enforcer = test_enforcer().await?;
		let shared_state = build_shared_state(cfg, pool, storage, Arc::new(RwLock::new(enforcer)))?;
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
		let response = self.app.clone().oneshot(request).await.expect("router request failed");
		let (parts, body) = response.into_parts();
		let body = to_bytes(body, BODY_MAX_SIZE_LIMIT_BYTES).await?;

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

	async fn upload_location(
		&self,
		cookie: Option<&str>,
		object_name: &str,
		latitude: &str,
		longitude: &str,
		content_type: &str,
		body: &[u8],
	) -> anyhow::Result<TestResponse> {
		let boundary = format!("memory-map-boundary-{}", unique_suffix()?);
		let multipart =
			multipart_body(&boundary, object_name, latitude, longitude, content_type, body);

		let mut request = Request::builder()
			.method("POST")
			.uri("/api/locations/")
			.header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"));

		if let Some(cookie) = cookie {
			request = request.header(header::COOKIE, cookie);
		}

		self.request(request.body(Body::from(multipart))?).await
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

	let response = app
		.upload_location(
			None,
			&object_name,
			"12.5",
			"-45.25",
			"image/svg+xml",
			b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
		)
		.await?;

	assert_eq!(response.status, StatusCode::UNAUTHORIZED);
	assert_eq!(response.text()?, "Unauthorized");
	assert_eq!(app.object_count(&object_name).await?, 0);
	assert!(app.state.storage.object_content_type(&object_name).await.is_err());

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
	assert_eq!(me["data"]["me"]["email"], user.email);

	let anonymous_me = app.graphql("query Me { me { email } }", json!({}), None).await?;
	assert_eq!(anonymous_me.status, StatusCode::OK);
	let anonymous_me = anonymous_me.json()?;
	assert_graphql_success(&anonymous_me)?;
	assert!(anonymous_me["data"]["me"].is_null());

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

	let upload = app
		.upload_location(Some(&user.cookie), &object_name, "12.5", "-45.25", "image/svg+xml", body)
		.await?;
	assert_eq!(upload.status, StatusCode::OK);
	let upload = upload.json()?;
	let object_id = upload[0]["id"].as_str().context("upload response is missing object id")?;
	assert_eq!(upload[0]["name"], object_name);
	assert_eq!(app.object_count(&object_name).await?, 1);
	assert_eq!(app.state.storage.object_content_type(&object_name).await?, "image/svg+xml");

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
	let object = visible_objects["data"]["s3Objects"]
		.as_array()
		.context("s3Objects response is not an array")?
		.iter()
		.find(|object| object["name"] == object_name)
		.context("uploaded object is missing from s3Objects")?;
	assert_eq!(object["contentType"], "image/svg+xml");

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
	assert_eq!(app.object_count(&object_name).await?, 0);
	assert!(app.state.storage.object_content_type(&object_name).await.is_err());

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
		("latitude", "90.1", "-45.25", "not a valid latitude value"),
		("longitude", "12.5", "-180.1", "not a valid longitude value"),
	] {
		let object_name = format!("invalid-{label}-upload-{}.svg", unique_suffix()?);

		let response = app
			.upload_location(
				Some(&user.cookie),
				&object_name,
				latitude,
				longitude,
				"image/svg+xml",
				b"<svg xmlns=\"http://www.w3.org/2000/svg\" />",
			)
			.await?;

		assert_eq!(response.status, StatusCode::BAD_REQUEST);
		assert!(response.text()?.contains(expected_error));
		assert_eq!(app.object_count(&object_name).await?, 0);
		assert!(app.state.storage.object_content_type(&object_name).await.is_err());
	}

	Ok(())
}

fn test_config() -> anyhow::Result<Config> {
	let mut pg = deadpool_postgres::Config::new();
	pg.dbname = Some(env_or_default("PG__DBNAME", "db"));
	pg.host = Some(env_or_default("PG__HOST", "127.0.0.1"));
	pg.port = Some(env_or_default("PG__PORT", "5432").parse()?);

	Ok(Config {
		pg,
		enable_registration: true,
		smtp_host: "smtp.example.test".to_string(),
		smtp_user: "memory-map-test".to_string(),
		smtp_pass: "memory-map-test-password".to_string(),
		smtp_from: "noreply@example.test".to_string(),
		cookie_secret: env_or_default(
			"COOKIE_SECRET",
			"memory-map-local-test-cookie-secret-at-least-64-bytes-long-0001-extra",
		),
		frontend_url: env_or_default("FRONTEND_URL", "http://127.0.0.1:3000"),
		s3_endpoint_url: env_or_default("S3_ENDPOINT_URL", "http://127.0.0.1:9000/"),
		s3_access_key: env_or_default("S3_ACCESS_KEY", "memorymapdev"),
		s3_secret_key: env_or_default("S3_SECRET_KEY", "memorymapdevsecret"),
		s3_bucket_name: env_or_default("S3_BUCKET_NAME", "memory-map"),
		s3_region: env_or_default("S3_REGION", "us-east-1"),
		s3_force_path_style: parse_bool_env("S3_FORCE_PATH_STYLE", true)?,
		s3_presigned_url_ttl_seconds: env_or_default("S3_PRESIGNED_URL_TTL_SECONDS", "604800")
			.parse()?,
		server_host: "127.0.0.1".to_string(),
		server_port: 8000,
		cors_allowed_origins: env_or_default("CORS_ALLOWED_ORIGINS", "http://127.0.0.1:3000"),
	})
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

fn multipart_body(
	boundary: &str,
	object_name: &str,
	latitude: &str,
	longitude: &str,
	content_type: &str,
	body: &[u8],
) -> Vec<u8> {
	let mut multipart = Vec::new();
	append_field(&mut multipart, boundary, "latitude", latitude.as_bytes());
	append_field(&mut multipart, boundary, "longitude", longitude.as_bytes());
	append_field(&mut multipart, boundary, "made_on", b"2026-05-31T12:00:00Z");
	multipart.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
	multipart.extend_from_slice(
		format!(
			"Content-Disposition: form-data; name=\"files\"; filename=\"{object_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
		)
		.as_bytes(),
	);
	multipart.extend_from_slice(body);
	multipart.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
	multipart
}

fn append_field(
	multipart: &mut Vec<u8>,
	boundary: &str,
	name: &str,
	value: &[u8],
) {
	multipart.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
	multipart.extend_from_slice(
		format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
	);
	multipart.extend_from_slice(value);
	multipart.extend_from_slice(b"\r\n");
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
	let now = SystemTime::now().duration_since(UNIX_EPOCH)?;
	Ok(now.as_nanos().to_string())
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
