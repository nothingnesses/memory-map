use leptos::prelude::*;
use thiserror::Error;
use wasm_bindgen::JsValue;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum AppError {
	#[error("Network error: {0}")]
	Network(String),

	#[error("GraphQL error: {0}")]
	GraphQL(String),

	#[error("Validation error: {0}")]
	Validation(String),

	#[error("Authentication error: {0}")]
	Authentication(String),

	#[error("System error: {0}")]
	System(String),

	#[error("Not found")]
	NotFound,

	#[error("JS error: {0}")]
	Js(String),
}

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
			error_ctx.report(AppError::System(format!("Context missing: {}", name)));
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
