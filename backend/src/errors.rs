use {
	crate::constants::{
		ERR_CASBIN,
		ERR_CONFIG,
		ERR_CREATE_POOL,
		ERR_DB,
		ERR_EMAIL,
		ERR_EMAIL_ADDRESS,
		ERR_FORBIDDEN,
		ERR_HASHING,
		ERR_INTERNAL_SERVER,
		ERR_INVALID_NUMBER,
		ERR_IO,
		ERR_MIGRATION,
		ERR_MULTIPART,
		ERR_NOT_FOUND,
		ERR_POOL,
		ERR_SMTP,
		ERR_SYSTEM_TIME,
		ERR_UNAUTHORIZED,
		ERR_VALIDATION,
	},
	axum::{
		http::StatusCode,
		response::{
			IntoResponse,
			Response,
		},
	},
	std::fmt,
};

#[derive(Debug)]
pub enum AppError {
	Internal(anyhow::Error),
	Unauthorized,
	Forbidden,
	NotFound(String),
	Validation(String),
}

impl fmt::Display for AppError {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		match self {
			AppError::Internal(e) => write!(f, "{}: {}", ERR_INTERNAL_SERVER, e),
			AppError::Unauthorized => write!(f, "{}", ERR_UNAUTHORIZED),
			AppError::Forbidden => write!(f, "{}", ERR_FORBIDDEN),
			AppError::NotFound(msg) => write!(f, "{}{}", ERR_NOT_FOUND, msg),
			AppError::Validation(msg) => write!(f, "{}{}", ERR_VALIDATION, msg),
		}
	}
}

impl std::error::Error for AppError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			AppError::Internal(e) => e.source(),
			_ => None,
		}
	}
}

impl From<anyhow::Error> for AppError {
	fn from(err: anyhow::Error) -> Self {
		AppError::Internal(err)
	}
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
			AppError::Internal(_) => ERR_INTERNAL_SERVER.to_string(),
			AppError::Unauthorized => ERR_UNAUTHORIZED.to_string(),
			AppError::Forbidden => ERR_FORBIDDEN.to_string(),
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

impl From<argon2::password_hash::Error> for AppError {
	fn from(err: argon2::password_hash::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_HASHING, err))
	}
}

impl From<lettre::error::Error> for AppError {
	fn from(err: lettre::error::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_EMAIL, err))
	}
}

impl From<lettre::address::AddressError> for AppError {
	fn from(err: lettre::address::AddressError) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_EMAIL_ADDRESS, err))
	}
}

impl From<lettre::transport::smtp::Error> for AppError {
	fn from(err: lettre::transport::smtp::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_SMTP, err))
	}
}

impl From<std::num::ParseIntError> for AppError {
	fn from(err: std::num::ParseIntError) -> Self {
		AppError::Validation(format!("{}{}", ERR_INVALID_NUMBER, err))
	}
}

impl From<deadpool_postgres::CreatePoolError> for AppError {
	fn from(err: deadpool_postgres::CreatePoolError) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_CREATE_POOL, err))
	}
}

impl From<deadpool_postgres::PoolError> for AppError {
	fn from(err: deadpool_postgres::PoolError) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_POOL, err))
	}
}

impl From<tokio_postgres::Error> for AppError {
	fn from(err: tokio_postgres::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_DB, err))
	}
}

impl From<axum::extract::multipart::MultipartError> for AppError {
	fn from(err: axum::extract::multipart::MultipartError) -> Self {
		AppError::Validation(format!("{}{}", ERR_MULTIPART, err))
	}
}

impl From<casbin::Error> for AppError {
	fn from(err: casbin::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_CASBIN, err))
	}
}

impl From<config::ConfigError> for AppError {
	fn from(err: config::ConfigError) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_CONFIG, err))
	}
}

impl From<refinery::error::Error> for AppError {
	fn from(err: refinery::error::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_MIGRATION, err))
	}
}

impl From<std::time::SystemTimeError> for AppError {
	fn from(err: std::time::SystemTimeError) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_SYSTEM_TIME, err))
	}
}

impl From<std::io::Error> for AppError {
	fn from(err: std::io::Error) -> Self {
		AppError::Internal(anyhow::anyhow!("{}{}", ERR_IO, err))
	}
}
