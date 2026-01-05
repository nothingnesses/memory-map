use crate::constants::{
	ERR_AUTHENTICATION_PREFIX, ERR_CONTEXT_MISSING, ERR_GRAPHQL_PREFIX, ERR_JS_PREFIX,
	ERR_NETWORK_PREFIX, ERR_NOT_FOUND_MSG, ERR_SYSTEM_PREFIX, ERR_VALIDATION_PREFIX,
};
use leptos::prelude::*;
use std::fmt;
use wasm_bindgen::JsValue;

#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
	Network(String),
	GraphQL(String),
	Validation(String),
	Authentication(String),
	System(String),
	NotFound,
	Js(String),
}

impl fmt::Display for AppError {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		match self {
			AppError::Network(msg) => write!(f, "{}{}", ERR_NETWORK_PREFIX, msg),
			AppError::GraphQL(msg) => write!(f, "{}{}", ERR_GRAPHQL_PREFIX, msg),
			AppError::Validation(msg) => write!(f, "{}{}", ERR_VALIDATION_PREFIX, msg),
			AppError::Authentication(msg) => write!(f, "{}{}", ERR_AUTHENTICATION_PREFIX, msg),
			AppError::System(msg) => write!(f, "{}{}", ERR_SYSTEM_PREFIX, msg),
			AppError::NotFound => write!(f, "{}", ERR_NOT_FOUND_MSG),
			AppError::Js(msg) => write!(f, "{}{}", ERR_JS_PREFIX, msg),
		}
	}
}

impl std::error::Error for AppError {}

impl From<JsValue> for AppError {
	fn from(value: JsValue) -> Self {
		AppError::Js(format!("{:?}", value))
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
		Self { error: RwSignal::new(None) }
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
	T: Clone + Default + 'static,
{
	use_context_safe::<T>(name).unwrap_or_default()
}
