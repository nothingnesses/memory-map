use {
	async_graphql::ErrorExtensions,
	axum::{
		http::StatusCode,
		response::{
			IntoResponse,
			Response,
		},
	},
};

/// Application-level error categories that all backend operations funnel into.
///
/// Source-specific errors (database, S3, hashing, SMTP, config, casbin, etc.)
/// all become `AppError::Internal` via `anyhow::Error`. Variants exist only for
/// categories that the client needs to distinguish in the response. Site-specific
/// detail belongs in `.context("Failed to ...")` chains at call sites; it is no
/// longer baked into per-source-type prefix constants.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
	#[error("Internal server error: {0}")]
	Internal(#[from] anyhow::Error),
	#[error("Unauthorized")]
	Unauthorized,
	#[error("Forbidden")]
	Forbidden,
	#[error("Not found: {0}")]
	NotFound(String),
	#[error("Validation error: {0}")]
	Validation(String),
}

/// Stable, client-facing error categories surfaced via `extensions.code` on
/// GraphQL errors and via HTTP status on the REST endpoint. Wire format
/// uses SCREAMING_SNAKE_CASE strings so existing API consumers can match
/// against literal codes without parsing message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
	Internal,
	Unauthorized,
	Forbidden,
	NotFound,
	Validation,
}

impl ErrorCode {
	pub fn as_str(self) -> &'static str {
		match self {
			ErrorCode::Internal => "INTERNAL",
			ErrorCode::Unauthorized => "UNAUTHORIZED",
			ErrorCode::Forbidden => "FORBIDDEN",
			ErrorCode::NotFound => "NOT_FOUND",
			ErrorCode::Validation => "VALIDATION",
		}
	}

	pub fn parse(value: &str) -> Option<Self> {
		Some(match value {
			"INTERNAL" => ErrorCode::Internal,
			"UNAUTHORIZED" => ErrorCode::Unauthorized,
			"FORBIDDEN" => ErrorCode::Forbidden,
			"NOT_FOUND" => ErrorCode::NotFound,
			"VALIDATION" => ErrorCode::Validation,
			_ => return None,
		})
	}
}

impl AppError {
	pub fn status_code(&self) -> StatusCode {
		match self.code() {
			ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
			ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
			ErrorCode::Forbidden => StatusCode::FORBIDDEN,
			ErrorCode::NotFound => StatusCode::NOT_FOUND,
			ErrorCode::Validation => StatusCode::BAD_REQUEST,
		}
	}

	/// Stable category code that mirrors the HTTP status / GraphQL extension.
	pub fn code(&self) -> ErrorCode {
		match self {
			AppError::Internal(_) => ErrorCode::Internal,
			AppError::Unauthorized => ErrorCode::Unauthorized,
			AppError::Forbidden => ErrorCode::Forbidden,
			AppError::NotFound(_) => ErrorCode::NotFound,
			AppError::Validation(_) => ErrorCode::Validation,
		}
	}

	/// Message shown to clients. Internal errors are intentionally opaque to
	/// avoid leaking internal details; the full source is in tracing logs.
	pub fn client_message(&self) -> String {
		match self {
			AppError::Internal(_) => "Internal server error".to_string(),
			AppError::Unauthorized => "Unauthorized".to_string(),
			AppError::Forbidden => "Forbidden".to_string(),
			AppError::NotFound(msg) => msg.clone(),
			AppError::Validation(msg) => msg.clone(),
		}
	}

	/// Wraps the error into an async-graphql Error, populating `extensions.code`
	/// so clients can branch on the stable code instead of parsing the message.
	pub fn extend_graphql(self) -> async_graphql::Error {
		tracing::error!("GraphQL error: {:?}", self);
		let code = self.code().as_str();
		let message = self.client_message();
		async_graphql::Error::new(message).extend_with(|_, extensions| {
			extensions.set("code", code);
		})
	}
}

impl IntoResponse for AppError {
	fn into_response(self) -> Response {
		tracing::error!("{:?}", self);
		(self.status_code(), self.client_message()).into_response()
	}
}

impl From<async_graphql::Error> for AppError {
	fn from(err: async_graphql::Error) -> Self {
		let code = err
			.extensions
			.as_ref()
			.and_then(|extensions| extensions.get("code"))
			.and_then(|value| match value {
				async_graphql::Value::String(string) => Some(string.as_str()),
				_ => None,
			})
			.and_then(ErrorCode::parse);

		match code {
			Some(ErrorCode::Unauthorized) => AppError::Unauthorized,
			Some(ErrorCode::Forbidden) => AppError::Forbidden,
			Some(ErrorCode::NotFound) => AppError::NotFound(err.message),
			Some(ErrorCode::Validation) => AppError::Validation(err.message),
			Some(ErrorCode::Internal) | None => AppError::Internal(anyhow::anyhow!(err.message)),
		}
	}
}

// Bare `?` is convenient at call sites. Each source-specific error type that
// the code hits routinely gets an explicit From impl that funnels into
// AppError::Internal via anyhow. Per-type message prefixes are dropped:
// site-specific context lives in `.context("Failed to ...")` chains.
macro_rules! impl_into_internal {
	($($ty:ty),* $(,)?) => {
		$(
			impl From<$ty> for AppError {
				fn from(err: $ty) -> Self {
					AppError::Internal(anyhow::Error::new(err))
				}
			}
		)*
	};
}

impl_into_internal!(
	axum::extract::multipart::MultipartError,
	casbin::Error,
	config::ConfigError,
	deadpool_postgres::CreatePoolError,
	deadpool_postgres::PoolError,
	lettre::address::AddressError,
	lettre::error::Error,
	lettre::transport::smtp::Error,
	refinery::error::Error,
	std::io::Error,
	std::num::ParseIntError,
	std::time::SystemTimeError,
	tokio_postgres::Error,
);

// argon2::password_hash::Error doesn't implement std::error::Error, so it goes
// through Display rather than the std::error::Error blanket conversion.
impl From<argon2::password_hash::Error> for AppError {
	fn from(err: argon2::password_hash::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{err}"))
	}
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
	fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
		AppError::Internal(anyhow::anyhow!(err))
	}
}
