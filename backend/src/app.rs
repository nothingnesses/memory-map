use {
	crate::{
		AppState,
		CachedGraphqlResponse,
		Config,
		GraphqlMutationCacheEffect,
		GraphqlResponseCacheKey,
		SharedState,
		UserId,
		constants::{
			BODY_MAX_SIZE_LIMIT_BYTES,
			GRAPHQL_BODY_LIMIT_BYTES,
			GRAPHQL_RESPONSE_CACHE_MAX_CAPACITY_BYTES,
			GRAPHQL_RESPONSE_CACHE_TTL_SECONDS,
		},
		controllers::api::locations::post as post_locations,
		db::queries::SELECT_USER_EXISTS_QUERY,
		errors::AppError,
		graphiql,
		graphql::queries::{
			mutation::Mutation,
			query::Query,
		},
		storage::StorageClient,
	},
	async_graphql::{
		BatchResponse,
		EmptySubscription,
		Request as GraphqlRequestInner,
		Response as GraphqlResponseBody,
		Schema,
		parser::types::{
			DocumentOperations,
			OperationType,
			Selection,
		},
	},
	async_graphql_axum::GraphQLRequest,
	axum::{
		Router,
		body::{
			Body,
			Bytes,
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
			StatusCode,
			header,
			request::Parts,
		},
		response::{
			IntoResponse,
			Response,
		},
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
		sync::{
			Arc,
			atomic::AtomicU64,
		},
		time::Duration,
	},
	tokio::sync::RwLock,
	tower_http::{
		cors::{
			AllowOrigin,
			CorsLayer,
		},
		limit::RequestBodyLimitLayer,
	},
};

type BackendState = AppState<Manager, Object<Manager>>;
type BackendSharedState = SharedState<Manager, Object<Manager>>;
type BackendSchema = Schema<Query, Mutation, EmptySubscription>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GraphqlRequestOperationDetails {
	operation_type: OperationType,
	mutation_root_field_count: Option<usize>,
}

pub fn build_shared_state(
	cfg: Config,
	pool: Pool<Manager>,
	storage: StorageClient,
	enforcer: Arc<RwLock<Enforcer>>,
) -> Arc<BackendSharedState> {
	let graphql_response_cache = Cache::builder()
		.max_capacity(GRAPHQL_RESPONSE_CACHE_MAX_CAPACITY_BYTES)
		.weigher(|_key, value: &CachedGraphqlResponse| value.weight())
		.time_to_live(Duration::from_secs(GRAPHQL_RESPONSE_CACHE_TTL_SECONDS))
		.build();
	let key = Key::from(cfg.auth.cookie_secret.as_bytes());

	Arc::new(SharedState {
		pool,
		storage,
		graphql_response_cache_epoch: AtomicU64::new(0),
		graphql_response_cache,
		key,
		config: cfg,
		enforcer,
	})
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
				.route_layer(RequestBodyLimitLayer::new(GRAPHQL_BODY_LIMIT_BYTES))
				.with_state(app_state.clone()),
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

async fn authenticated_user_id(
	state: &BackendState,
	jar: &PrivateCookieJar,
) -> Result<Option<i64>, AppError> {
	let Some(user_id) = jar.get("auth_token").and_then(|cookie| cookie.value().parse::<i64>().ok())
	else {
		return Ok(None);
	};
	let client = state.inner.pool.get().await?;
	let statement = client.prepare_cached(SELECT_USER_EXISTS_QUERY).await?;
	Ok(client.query_opt(&statement, &[&user_id]).await?.map(|_| user_id))
}

fn graphql_request_operation_details(
	request: &mut GraphqlRequestInner
) -> Option<GraphqlRequestOperationDetails> {
	let operation_name = request.operation_name.clone();
	let document = request.parsed_query().ok()?;

	let operation = match &document.operations {
		DocumentOperations::Single(operation) => operation,
		DocumentOperations::Multiple(operations) => {
			let operation_name = operation_name?;
			operations.get(operation_name.as_str())?
		}
	};
	let operation_type = operation.node.ty;
	let mutation_root_field_count = if matches!(operation_type, OperationType::Mutation) {
		mutation_root_field_count(operation)
	} else {
		None
	};

	Some(GraphqlRequestOperationDetails {
		operation_type,
		mutation_root_field_count,
	})
}

#[cfg(test)]
fn graphql_request_operation_type(request: &mut GraphqlRequestInner) -> Option<OperationType> {
	graphql_request_operation_details(request).map(|details| details.operation_type)
}

fn mutation_root_field_count(
	operation: &async_graphql::Positioned<async_graphql::parser::types::OperationDefinition>
) -> Option<usize> {
	let mut field_count = 0;
	for selection in &operation.node.selection_set.node.items {
		match &selection.node {
			Selection::Field(_) => {
				field_count += 1;
			}
			Selection::FragmentSpread(_) | Selection::InlineFragment(_) => return None,
		}
	}
	Some(field_count)
}

fn graphql_mutation_should_invalidate_cache(
	operation_type: Option<OperationType>,
	mutation_root_field_count: Option<usize>,
	cache_effect: &GraphqlMutationCacheEffect,
) -> bool {
	if !matches!(operation_type, Some(OperationType::Mutation)) {
		return false;
	}

	!matches!(
		mutation_root_field_count,
		Some(field_count)
			if field_count > 0 && cache_effect.non_invalidating_field_count() == field_count
	)
}

fn hash_cache_component(
	hasher: &mut blake3::Hasher,
	bytes: &[u8],
) {
	hasher.update(&(bytes.len() as u64).to_le_bytes());
	hasher.update(bytes);
}

fn graphql_response_cache_key(
	request: &GraphqlRequestInner,
	user_id: Option<i64>,
	authorization: Option<&HeaderValue>,
	cache_epoch: u64,
) -> Option<GraphqlResponseCacheKey> {
	if !request.uploads.is_empty() || !request.extensions.is_empty() {
		return None;
	}

	let mut hasher = blake3::Hasher::new();
	hash_cache_component(&mut hasher, b"memory-map/graphql-response-cache/v1");
	hasher.update(&cache_epoch.to_le_bytes());

	match user_id {
		Some(user_id) => {
			hasher.update(&[1]);
			hasher.update(&user_id.to_le_bytes());
		}
		None => {
			hasher.update(&[0]);
		}
	};
	if let Some(authorization) = authorization {
		hasher.update(&[1]);
		hash_cache_component(&mut hasher, authorization.as_bytes());
	} else {
		hasher.update(&[0]);
	}

	hash_cache_component(&mut hasher, request.query.as_bytes());
	if let Some(operation_name) = &request.operation_name {
		hasher.update(&[1]);
		hash_cache_component(&mut hasher, operation_name.as_bytes());
	} else {
		hasher.update(&[0]);
	}
	let variables = serde_json::to_vec(&request.variables).ok()?;
	hash_cache_component(&mut hasher, &variables);

	let mut bytes = [0; 32];
	bytes.copy_from_slice(hasher.finalize().as_bytes());
	Some(GraphqlResponseCacheKey::new(bytes))
}

fn graphql_response_to_cache_entry(
	response: GraphqlResponseBody
) -> Result<CachedGraphqlResponse, serde_json::Error> {
	let batch_response = BatchResponse::from(response);
	let mut headers = HeaderMap::new();
	headers.insert(
		header::CONTENT_TYPE,
		HeaderValue::from_static("application/graphql-response+json"),
	);
	if batch_response.is_ok() &&
		let Some(cache_control) = batch_response.cache_control().value() &&
		let Ok(value) = HeaderValue::from_str(&cache_control)
	{
		headers.insert(header::CACHE_CONTROL, value);
	}
	headers.extend(batch_response.http_headers());
	let bytes = Bytes::from(serde_json::to_vec(&batch_response)?);

	Ok(CachedGraphqlResponse {
		status: StatusCode::OK,
		headers,
		bytes,
	})
}

fn cache_entry_to_response(cached: CachedGraphqlResponse) -> Response {
	let mut response = Response::new(Body::from(cached.bytes));
	*response.status_mut() = cached.status;
	*response.headers_mut() = cached.headers;
	response
}

async fn graphql_handler(
	State(state): State<BackendState>,
	Extension(schema): Extension<BackendSchema>,
	headers: HeaderMap,
	jar: PrivateCookieJar,
	req: GraphQLRequest,
) -> (PrivateCookieJar, Response) {
	let mut req = req.into_inner();

	let user_id = match authenticated_user_id(&state, &jar).await {
		Ok(user_id) => user_id,
		Err(error) => return (jar, error.into_response()),
	};
	if let Some(user_id) = user_id {
		req = req.data(UserId(user_id));
	}

	let operation_details = graphql_request_operation_details(&mut req);
	let operation_type = operation_details.map(|details| details.operation_type);
	let mutation_root_field_count =
		operation_details.and_then(|details| details.mutation_root_field_count);
	let cache_epoch = state.inner.graphql_response_cache_epoch();
	let cache_key = if matches!(operation_type, Some(OperationType::Query)) {
		graphql_response_cache_key(&req, user_id, headers.get(header::AUTHORIZATION), cache_epoch)
	} else {
		None
	};
	if let Some(cache_key) = &cache_key &&
		let Some(cached_response) = state.inner.graphql_response_cache.get(cache_key).await
	{
		return (jar, cache_entry_to_response(cached_response));
	}

	// parking_lot::Mutex has no poisoning and is held only briefly around a vector
	// push during resolver execution, so the lock acquisition is infallible.
	let cookies = Arc::new(parking_lot::Mutex::new(Vec::<Cookie<'static>>::new()));
	req = req.data(cookies.clone());
	let mutation_cache_effect = Arc::new(GraphqlMutationCacheEffect::default());
	req = req.data(mutation_cache_effect.clone());

	let response = schema.execute(req).await;
	let response_is_ok = response.is_ok();
	let cached_response = match graphql_response_to_cache_entry(response) {
		Ok(cached_response) => cached_response,
		Err(error) => {
			tracing::error!("Failed to serialize GraphQL response: {:?}", error);
			return (jar, StatusCode::INTERNAL_SERVER_ERROR.into_response());
		}
	};

	if graphql_mutation_should_invalidate_cache(
		operation_type,
		mutation_root_field_count,
		&mutation_cache_effect,
	) {
		state.inner.invalidate_graphql_response_cache();
	}

	let mut jar = jar;
	let issued_cookies = cookies.lock().clone();
	for cookie in &issued_cookies {
		jar = jar.add(cookie.clone());
	}

	if let Some(cache_key) = cache_key &&
		response_is_ok &&
		issued_cookies.is_empty() &&
		!cached_response.headers.contains_key(header::SET_COOKIE) &&
		state.inner.graphql_response_cache_epoch() == cache_epoch
	{
		state.inner.graphql_response_cache.insert(cache_key, cached_response.clone()).await;
	}

	(jar, cache_entry_to_response(cached_response))
}

#[cfg(test)]
mod tests {
	use {
		super::*,
		async_graphql::{
			Value,
			Variables,
		},
		serde_json::json,
	};

	fn request_with_variable(id: i64) -> GraphqlRequestInner {
		GraphqlRequestInner::new("query Object($id: Int!) { s3ObjectById(id: $id) { id } }")
			.variables(Variables::from_json(json!({ "id": id })))
	}

	fn cache_key(
		request: &GraphqlRequestInner,
		user_id: Option<i64>,
		cache_epoch: u64,
	) -> anyhow::Result<GraphqlResponseCacheKey> {
		graphql_response_cache_key(request, user_id, None, cache_epoch)
			.ok_or_else(|| anyhow::anyhow!("request should be cacheable"))
	}

	#[test]
	fn graphql_operation_type_uses_selected_named_operation() {
		let mut query = GraphqlRequestInner::new(
			"query Read { config { enableRegistration } } mutation Write { logout }",
		)
		.operation_name("Read");
		assert_eq!(graphql_request_operation_type(&mut query), Some(OperationType::Query));

		let mut mutation = GraphqlRequestInner::new(
			"query Read { config { enableRegistration } } mutation Write { logout }",
		)
		.operation_name("Write");
		assert_eq!(graphql_request_operation_type(&mut mutation), Some(OperationType::Mutation));
	}

	#[test]
	fn graphql_operation_type_skips_ambiguous_multi_operation_documents() {
		let mut request = GraphqlRequestInner::new(
			"query Read { config { enableRegistration } } mutation Write { logout }",
		);
		assert_eq!(graphql_request_operation_type(&mut request), None);
	}

	#[test]
	fn graphql_operation_details_count_mutation_root_fields() {
		let mut request = GraphqlRequestInner::new("mutation { a: logout b: logout }");

		assert_eq!(
			graphql_request_operation_details(&mut request),
			Some(GraphqlRequestOperationDetails {
				operation_type: OperationType::Mutation,
				mutation_root_field_count: Some(2),
			})
		);
	}

	#[test]
	fn graphql_operation_details_treat_fragment_root_mutation_selection_as_uncountable() {
		let mut request = GraphqlRequestInner::new(
			"mutation { ...WriteFields } fragment WriteFields on Mutation { logout }",
		);

		assert_eq!(
			graphql_request_operation_details(&mut request),
			Some(GraphqlRequestOperationDetails {
				operation_type: OperationType::Mutation,
				mutation_root_field_count: None,
			})
		);
	}

	#[test]
	fn graphql_mutation_cache_effect_only_suppresses_when_all_root_fields_marked() {
		let cache_effect = GraphqlMutationCacheEffect::default();
		assert!(!graphql_mutation_should_invalidate_cache(
			Some(OperationType::Query),
			None,
			&cache_effect,
		));
		assert!(graphql_mutation_should_invalidate_cache(
			Some(OperationType::Mutation),
			Some(2),
			&cache_effect,
		));

		cache_effect.mark_non_invalidating_field();
		assert!(graphql_mutation_should_invalidate_cache(
			Some(OperationType::Mutation),
			Some(2),
			&cache_effect,
		));

		cache_effect.mark_non_invalidating_field();
		assert!(!graphql_mutation_should_invalidate_cache(
			Some(OperationType::Mutation),
			Some(2),
			&cache_effect,
		));
		assert!(graphql_mutation_should_invalidate_cache(
			Some(OperationType::Mutation),
			None,
			&cache_effect,
		));
	}

	#[test]
	fn graphql_response_cache_key_scopes_by_actor_variables_and_epoch() -> anyhow::Result<()> {
		let key = cache_key(&request_with_variable(1), Some(10), 0)?;
		assert_eq!(cache_key(&request_with_variable(1), Some(10), 0)?, key);
		assert_ne!(cache_key(&request_with_variable(1), Some(11), 0)?, key);
		assert_ne!(cache_key(&request_with_variable(2), Some(10), 0)?, key);
		assert_ne!(cache_key(&request_with_variable(1), Some(10), 1)?, key);
		Ok(())
	}

	#[test]
	fn graphql_response_cache_key_skips_requests_with_extensions() {
		let mut request = request_with_variable(1);
		request.extensions.insert("cacheBypass".to_string(), Value::Boolean(true));
		assert!(graphql_response_cache_key(&request, Some(10), None, 0).is_none());
	}

	#[test]
	fn graphql_response_cache_key_scopes_by_authorization_header() -> anyhow::Result<()> {
		let request = request_with_variable(1);
		let bearer_a = HeaderValue::from_static("Bearer a");
		let bearer_b = HeaderValue::from_static("Bearer b");

		let key = graphql_response_cache_key(&request, Some(10), Some(&bearer_a), 0)
			.ok_or_else(|| anyhow::anyhow!("request should be cacheable"))?;

		assert_eq!(
			graphql_response_cache_key(&request, Some(10), Some(&bearer_a), 0)
				.ok_or_else(|| anyhow::anyhow!("request should be cacheable"))?,
			key
		);
		assert_ne!(
			graphql_response_cache_key(&request, Some(10), Some(&bearer_b), 0)
				.ok_or_else(|| anyhow::anyhow!("request should be cacheable"))?,
			key
		);
		assert_ne!(cache_key(&request, Some(10), 0)?, key);
		Ok(())
	}
}
