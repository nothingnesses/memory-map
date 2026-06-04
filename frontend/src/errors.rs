use {
	crate::constants::ERR_CONTEXT_MISSING,
	leptos::prelude::*,
	wasm_bindgen::JsValue,
};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AppError {
	#[error("Network error: {0}")]
	Network(String),
	#[error("GraphQL error: {0}")]
	GraphQL(String),
	#[error("Validation error: {0}")]
	Validation(String),
	#[error("Authentication error: {0}")]
	Authentication(String),
	#[error("Forbidden: {0}")]
	Forbidden(String),
	#[error("System error: {0}")]
	System(String),
	#[error("Not found")]
	NotFound,
	#[error("JS error: {0}")]
	Js(String),
}

impl AppError {
	/// Maps a GraphQL response's first error (if any) to the matching variant,
	/// reading the backend's stable `extensions.code`. Falls back to
	/// `AppError::GraphQL` when no code is present or the code is unknown.
	pub fn from_graphql_errors(errors: &[graphql_client::Error]) -> Option<Self> {
		let error = errors.first()?;
		let code = error
			.extensions
			.as_ref()
			.and_then(|ext| ext.get("code"))
			.and_then(|value| value.as_str());
		Some(match code {
			Some("UNAUTHORIZED") => AppError::Authentication(error.message.clone()),
			Some("FORBIDDEN") => AppError::Forbidden(error.message.clone()),
			Some("NOT_FOUND") => AppError::NotFound,
			Some("VALIDATION") => AppError::Validation(error.message.clone()),
			_ => AppError::GraphQL(error.message.clone()),
		})
	}
}

/// Unwraps the `data` of a GraphQL response, surfacing any backend errors as
/// the matching `AppError` variant. Use at the boundary of every `run()`
/// function so the frontend's error UX is consistent with the backend's
/// stable error codes.
pub fn graphql_data<T>(response: graphql_client::Response<T>) -> Result<T, AppError> {
	if let Some(errors) = response.errors.as_ref() &&
		!errors.is_empty()
	{
		return Err(AppError::from_graphql_errors(errors)
			.unwrap_or_else(|| AppError::GraphQL("Unknown error".to_string())));
	}
	response.data.ok_or_else(|| AppError::GraphQL("Empty response".to_string()))
}

impl From<JsValue> for AppError {
	fn from(value: JsValue) -> Self {
		AppError::Js(format!("{:?}", value))
	}
}

// Network and serialization errors from the GraphQL HTTP client come through as
// a boxed std::error::Error (for the authed helper) or a bare reqwest::Error
// (for the unauthed helper); both are treated as Network failures so the UI can
// distinguish them from server-reported GraphQL errors.
impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
	fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
		AppError::Network(err.to_string())
	}
}

impl From<reqwest::Error> for AppError {
	fn from(err: reqwest::Error) -> Self {
		AppError::Network(err.to_string())
	}
}

#[derive(Clone, Copy)]
pub struct ErrorContext {
	pub error: RwSignal<Option<AppError>>,
}

impl Default for ErrorContext {
	fn default() -> Self {
		Self::new()
	}
}

impl ErrorContext {
	pub fn new() -> Self {
		Self {
			error: RwSignal::new(None),
		}
	}

	pub fn report(
		&self,
		err: impl Into<AppError>,
	) {
		self.error.set(Some(err.into()));
	}

	pub fn clear(&self) {
		self.error.set(None);
	}
}

pub fn provide_error_context() {
	provide_context(ErrorContext::new());
}

pub fn use_error_context() -> ErrorContext {
	use_context::<ErrorContext>().unwrap_or_else(|| {
		let ctx = ErrorContext::new();
		provide_context(ctx);
		ctx
	})
}

/// Safely retrieve a context or report a system error if it's missing.
pub fn use_context_safe<T: Clone + 'static>(name: &str) -> Option<T> {
	match use_context::<T>() {
		Some(ctx) => Some(ctx),
		None => {
			let error_ctx = use_error_context();
			error_ctx.report(AppError::System(format!("{}{}", ERR_CONTEXT_MISSING, name)));
			None
		}
	}
}

/// Expect a context to be present, or report a system error and return None.
/// This is a safer alternative to .expect() or .unwrap() on use_context.
pub fn expect_context_safe<T>(name: &str) -> T
where
	T: Clone + Default + 'static, {
	use_context_safe::<T>(name).unwrap_or_default()
}
