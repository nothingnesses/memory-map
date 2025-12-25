use crate::SharedState;
use crate::graphql::queries::mutation::Mutation;
use axum::{
	extract::{Json, Path, State},
	http::StatusCode,
	response::IntoResponse,
};
use axum_macros::debug_handler;
use deadpool::managed::Object;
use deadpool_postgres::Manager;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct DeleteS3ObjectsRequest {
	s3_objects: Vec<i64>,
}

// @todo Modify to return both status code and object deleted if succesful.
#[debug_handler]
pub async fn delete(
	State(state): State<Arc<SharedState<Manager, Object<Manager>>>>,
	Path(id): Path<i64>,
) -> impl IntoResponse {
	let client = match state.pool.get().await {
		Ok(client) => client,
		Err(e) => {
			tracing::error!("Failed to get DB client: {}", e);
			return StatusCode::INTERNAL_SERVER_ERROR;
		}
	};

	match Mutation::delete_s3_objects_worker(
		&client,
		&state.minio_client,
		&state.bucket_name,
		&[id],
	)
	.await
	{
		Ok(_) => {
			state.update_last_modified();
			StatusCode::OK
		}
		Err(e) => {
			tracing::error!("Failed to delete object: {:?}", e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	}
}

// @todo Modify to return both status code and objects deleted if succesful.
#[debug_handler]
pub async fn delete_many(
	State(state): State<Arc<SharedState<Manager, Object<Manager>>>>,
	Json(payload): Json<DeleteS3ObjectsRequest>,
) -> impl IntoResponse {
	let client = match state.pool.get().await {
		Ok(client) => client,
		Err(e) => {
			tracing::error!("Failed to get DB client: {}", e);
			return StatusCode::INTERNAL_SERVER_ERROR;
		}
	};

	match Mutation::delete_s3_objects_worker(
		&client,
		&state.minio_client,
		&state.bucket_name,
		&payload.s3_objects,
	)
	.await
	{
		Ok(_) => {
			state.update_last_modified();
			StatusCode::OK
		}
		Err(e) => {
			tracing::error!("Failed to delete objects: {:?}", e);
			StatusCode::INTERNAL_SERVER_ERROR
		}
	}
}
