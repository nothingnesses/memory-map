use axum::{
	http::StatusCode,
	response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
	#[error("Internal server error")]
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

impl AppError {
	pub fn status_code(&self) -> StatusCode {
		match self {
			AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
			AppError::Unauthorized => StatusCode::UNAUTHORIZED,
			AppError::Forbidden => StatusCode::FORBIDDEN,
			AppError::NotFound(_) => StatusCode::NOT_FOUND,
			AppError::Validation(_) => StatusCode::BAD_REQUEST,
		}
	}

	pub fn client_message(&self) -> String {
		match self {
			AppError::Internal(_) => "Internal server error".to_string(),
			AppError::Unauthorized => "Unauthorized".to_string(),
			AppError::Forbidden => "Forbidden".to_string(),
			AppError::NotFound(msg) => msg.clone(),
			AppError::Validation(msg) => msg.clone(),
		}
	}

	pub fn extend_graphql(self) -> async_graphql::Error {
		tracing::error!("GraphQL error: {:?}", self);
		async_graphql::Error::new(self.to_string())
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
		AppError::Internal(anyhow::anyhow!(err.message))
	}
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
	fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
		AppError::Internal(anyhow::anyhow!(err))
	}
}

impl From<minio::s3::error::Error> for AppError {
	fn from(err: minio::s3::error::Error) -> Self {
		AppError::Internal(anyhow::anyhow!(err))
	}
}

impl From<argon2::password_hash::Error> for AppError {
	fn from(err: argon2::password_hash::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("Hashing error: {}", err))
	}
}

impl From<lettre::error::Error> for AppError {
	fn from(err: lettre::error::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("Email error: {}", err))
	}
}

impl From<lettre::address::AddressError> for AppError {
	fn from(err: lettre::address::AddressError) -> Self {
		AppError::Internal(anyhow::anyhow!("Email address error: {}", err))
	}
}

impl From<lettre::transport::smtp::Error> for AppError {
	fn from(err: lettre::transport::smtp::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("SMTP error: {}", err))
	}
}

impl From<std::num::ParseIntError> for AppError {
	fn from(err: std::num::ParseIntError) -> Self {
		AppError::Validation(format!("Invalid number format: {}", err))
	}
}

impl From<deadpool_postgres::CreatePoolError> for AppError {
	fn from(err: deadpool_postgres::CreatePoolError) -> Self {
		AppError::Internal(anyhow::anyhow!("Failed to create pool: {}", err))
	}
}

impl From<deadpool_postgres::PoolError> for AppError {
	fn from(err: deadpool_postgres::PoolError) -> Self {
		AppError::Internal(anyhow::anyhow!("Pool error: {}", err))
	}
}

impl From<tokio_postgres::Error> for AppError {
	fn from(err: tokio_postgres::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("Database error: {}", err))
	}
}

impl From<axum::extract::multipart::MultipartError> for AppError {
	fn from(err: axum::extract::multipart::MultipartError) -> Self {
		AppError::Validation(format!("Multipart error: {}", err))
	}
}

impl From<casbin::Error> for AppError {
	fn from(err: casbin::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("Casbin error: {}", err))
	}
}

impl From<config::ConfigError> for AppError {
	fn from(err: config::ConfigError) -> Self {
		AppError::Internal(anyhow::anyhow!("Config error: {}", err))
	}
}

impl From<refinery::error::Error> for AppError {
	fn from(err: refinery::error::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("Migration error: {}", err))
	}
}

impl From<std::time::SystemTimeError> for AppError {
	fn from(err: std::time::SystemTimeError) -> Self {
		AppError::Internal(anyhow::anyhow!("System time error: {}", err))
	}
}

impl From<std::io::Error> for AppError {
	fn from(err: std::io::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("IO error: {}", err))
	}
}
