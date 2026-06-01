use {
	crate::{
		AppState,
		CachedResponse,
		Config,
		SharedState,
		UserId,
		constants::{
			BODY_MAX_SIZE_LIMIT_BYTES,
			CACHE_MAX_CAPACITY_BYTES,
			CACHE_TTL_SECONDS,
			GRAPHQL_BODY_LIMIT_BYTES,
		},
		controllers::api::locations::post as post_locations,
		db::queries::SELECT_USER_EXISTS_QUERY,
		graphiql,
		graphql::queries::{
			mutation::Mutation,
			query::Query,
		},
		storage::StorageClient,
	},
	anyhow::Context,
	async_graphql::{
		EmptySubscription,
		Schema,
	},
	async_graphql_axum::{
		GraphQLRequest,
		GraphQLResponse,
	},
	axum::{
		Router,
		body::{
			Body,
			to_bytes,
		},
		extract::{
			DefaultBodyLimit,
			Extension,
			State,
		},
		http::{
			HeaderMap,
			HeaderValue,
			Method,
			Request,
			StatusCode,
			header,
			request::Parts,
		},
		middleware::{
			self,
			Next,
		},
		response::IntoResponse,
		routing::{
			get,
			post,
		},
	},
	axum_extra::extract::cookie::{
		Cookie,
		Key,
		PrivateCookieJar,
	},
	casbin::Enforcer,
	deadpool::managed::{
		Object,
		Pool,
	},
	deadpool_postgres::Manager,
	moka::future::Cache,
	std::{
		collections::hash_map::DefaultHasher,
		hash::{
			Hash,
			Hasher,
		},
		sync::{
			Arc,
			Mutex,
			atomic::{
				AtomicU64,
				Ordering,
			},
		},
		time::{
			Duration,
			SystemTime,
			UNIX_EPOCH,
		},
	},
	tokio::sync::RwLock,
	tower_http::cors::{
		AllowOrigin,
		CorsLayer,
	},
};

type BackendState = AppState<Manager, Object<Manager>>;
type BackendSharedState = SharedState<Manager, Object<Manager>>;
type BackendSchema = Schema<Query, Mutation, EmptySubscription>;

pub fn build_shared_state(
	cfg: Config,
	pool: Pool<Manager>,
	storage: StorageClient,
	enforcer: Arc<RwLock<Enforcer>>,
) -> anyhow::Result<Arc<BackendSharedState>> {
	let last_modified = AtomicU64::new(
		SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.context("System time is before UNIX EPOCH")?
			.as_millis() as u64,
	);

	let response_cache = Cache::builder()
		.max_capacity(CACHE_MAX_CAPACITY_BYTES)
		.weigher(|_key, value: &CachedResponse| value.weight())
		.time_to_live(Duration::from_secs(CACHE_TTL_SECONDS))
		.build();

	let key = Key::from(cfg.auth.cookie_secret.as_bytes());

	Ok(Arc::new(SharedState {
		pool,
		storage,
		last_modified,
		response_cache,
		key,
		config: cfg,
		enforcer,
	}))
}

pub fn build_app(shared_state: Arc<BackendSharedState>) -> Router {
	let app_state = AppState {
		inner: shared_state.clone(),
	};
	let schema = build_schema(shared_state.clone());
	let cors = cors_layer(&shared_state.config);
	let key = shared_state.key.clone();

	Router::new()
		.route("/", get(graphiql))
		.route(
			"/",
			post(graphql_handler)
				.with_state(app_state.clone())
				.layer(middleware::from_fn_with_state(app_state.clone(), caching_middleware)),
		)
		.route(
			"/api/locations/",
			post(post_locations)
				.route_layer(DefaultBodyLimit::max(BODY_MAX_SIZE_LIMIT_BYTES))
				.with_state(app_state),
		)
		.layer(Extension(schema))
		.layer(Extension(key))
		.route_layer(cors)
}

fn build_schema(shared_state: Arc<BackendSharedState>) -> BackendSchema {
	Schema::build(Query, Mutation, EmptySubscription)
		.data(shared_state.key.clone())
		.data(shared_state)
		.finish()
}

fn cors_layer(config: &Config) -> CorsLayer {
	let frontend_url = config.frontend.url.clone();
	let cors_allowed_origins = config.cors.allowed_origins.clone();

	CorsLayer::new()
		.allow_origin(AllowOrigin::predicate(
			move |origin: &HeaderValue, _request_parts: &Parts| {
				let origin_bytes = origin.as_bytes();
				origin_bytes == frontend_url.as_bytes() ||
					origin_bytes == cors_allowed_origins.as_bytes()
			},
		))
		.allow_methods([Method::GET, Method::POST])
		.allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
		.allow_credentials(true)
}

async fn caching_middleware(
	State(state): State<BackendState>,
	_headers: HeaderMap,
	request: Request<Body>,
	next: Next,
) -> axum::response::Result<impl IntoResponse> {
	// 1. Read body
	let (parts, body) = request.into_parts();
	// Limit body size to avoid DoS.
	let bytes = match to_bytes(body, GRAPHQL_BODY_LIMIT_BYTES).await {
		Ok(b) => b,
		Err(_) => return Ok(StatusCode::PAYLOAD_TOO_LARGE.into_response()),
	};

	// Return early if it's a mutation.
	// Mutations change state and should not be cached. Caching them would prevent
	// the server from executing the mutation on subsequent requests. GraphQL keywords
	// are case-sensitive per spec, so a literal lowercase prefix match is enough; no
	// need to lowercase the entire query string on every request.
	if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) &&
		let Some(query) = json.get("query").and_then(|q| q.as_str()) &&
		query.trim_start().starts_with("mutation")
	{
		let req = Request::from_parts(parts, Body::from(bytes));
		return Ok(next.run(req).await.into_response());
	}

	// 2. Hash body
	let mut hasher = DefaultHasher::new();
	bytes.hash(&mut hasher);
	if let Some(auth_header) = parts.headers.get("Authorization") {
		auth_header.hash(&mut hasher);
	}
	if let Some(cookie_header) = parts.headers.get(header::COOKIE) {
		cookie_header.hash(&mut hasher);
	}
	let hash = hasher.finish();

	// 3. Check cache
	if let Some(cached) = state.inner.response_cache.get(&hash).await {
		tracing::info!("Cache hit for hash: {}", hash);
		let mut response = axum::response::Response::new(Body::from(cached.bytes));
		*response.status_mut() = cached.status;
		if let Some(content_type) = cached.content_type {
			response.headers_mut().insert(header::CONTENT_TYPE, content_type);
		}

		let last_modified = state.inner.last_modified.load(Ordering::Relaxed);
		let etag = format!("\"{last_modified}\"");
		response.headers_mut().insert(
			"ETag",
			HeaderValue::from_str(&etag).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
		);

		return Ok(response);
	}

	// 4. Process request
	let req = Request::from_parts(parts, Body::from(bytes));
	let response = next.run(req).await;

	// 5. Cache response (only successful GraphQL responses with no errors)
	let (parts, body) = response.into_parts();
	let bytes = match to_bytes(body, BODY_MAX_SIZE_LIMIT_BYTES).await {
		Ok(b) => b,
		Err(_) => return Ok(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
	};

	if should_cache_response(parts.status, &bytes) {
		let cached = CachedResponse {
			status: parts.status,
			content_type: parts.headers.get(header::CONTENT_TYPE).cloned(),
			bytes: bytes.clone(),
		};
		state.inner.response_cache.insert(hash, cached).await;
	}

	let mut response = axum::response::Response::from_parts(parts, Body::from(bytes));

	let last_modified = state.inner.last_modified.load(Ordering::Relaxed);
	let etag = format!("\"{last_modified}\"");
	response.headers_mut().insert(
		"ETag",
		HeaderValue::from_str(&etag).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
	);

	Ok(response)
}

/// Whether a GraphQL response is safe to cache.
///
/// Skips non-2xx and any response whose JSON body contains a top-level `errors` field,
/// since partial-failure responses are intentionally heterogeneous over time.
fn should_cache_response(
	status: StatusCode,
	body: &[u8],
) -> bool {
	if !status.is_success() {
		return false;
	}
	let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) else {
		return false;
	};
	json.get("errors").is_none()
}

async fn graphql_handler(
	State(state): State<BackendState>,
	Extension(schema): Extension<BackendSchema>,
	jar: PrivateCookieJar,
	req: GraphQLRequest,
) -> (PrivateCookieJar, GraphQLResponse) {
	let mut req = req.into_inner();

	if let Some(cookie) = jar.get("auth_token") &&
		let Ok(user_id) = cookie.value().parse::<i64>()
	{
		// Verify user exists in database
		let user_exists = if let Ok(client) = state.inner.pool.get().await {
			match client.prepare_cached(SELECT_USER_EXISTS_QUERY).await {
				Ok(stmt) => matches!(client.query_opt(&stmt, &[&user_id]).await, Ok(Some(_))),
				Err(_) => false,
			}
		} else {
			false
		};

		if user_exists {
			req = req.data(UserId(user_id));
		}
	}

	let cookies = Arc::new(Mutex::new(Vec::<Cookie<'static>>::new()));
	req = req.data(cookies.clone());

	let response = schema.execute(req).await;

	let mut jar = jar;
	if let Ok(cookies) = cookies.lock() {
		for cookie in cookies.iter() {
			jar = jar.add(cookie.clone());
		}
	}

	(jar, response.into())
}
