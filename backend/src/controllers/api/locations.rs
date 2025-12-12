// @todo Use minio::s3::Client::upload_part to do multipart upload

use crate::SharedState;
use crate::graphql::objects::location::Location;
use crate::graphql::queries::mutation::Mutation;
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_macros::debug_handler;
use deadpool::managed::Object;
use deadpool_postgres::Manager;
use minio::s3::segmented_bytes::SegmentedBytes;
use minio::s3::types::S3Api;
use std::sync::Arc;

#[derive(Debug)]
struct FileData {
	filename: String,
	content_type: String,
	bytes: Bytes,
}

#[debug_handler]
pub async fn post(
	State(state): State<Arc<SharedState<Manager, Object<Manager>>>>,
	mut multipart: Multipart,
) -> impl IntoResponse {
	let mut latitude: Option<f64> = None;
	let mut longitude: Option<f64> = None;
	let mut files: Vec<FileData> = Vec::new();

	while let Some(field) = multipart.next_field().await.unwrap() {
		let name = field.name().unwrap().to_string();

		match name.as_str() {
			"latitude" => {
				if let Ok(txt) = field.text().await {
					if let Ok(val) = txt.parse::<f64>() {
						latitude = Some(val);
					}
				}
			}
			"longitude" => {
				if let Ok(txt) = field.text().await {
					if let Ok(val) = txt.parse::<f64>() {
						longitude = Some(val);
					}
				}
			}
			"files" => {
				let filename = field.file_name().unwrap_or_default().to_string();
				let content_type = field.content_type().unwrap_or_default().to_string();
				if let Ok(bytes) = field.bytes().await {
					files.push(FileData { filename, content_type, bytes });
				}
			}
			_ => {}
		}
	}

	tracing::debug!("Received Location:");
	tracing::debug!("Latitude: {:?}", latitude);
	tracing::debug!("Longitude: {:?}", longitude);
	tracing::debug!("Files: {} uploaded", files.len());
	for file in files {
		tracing::debug!(
			" - Name: {}, Type: {}, Size: {} bytes",
			file.filename,
			file.content_type,
			file.bytes.len()
		);
		let _ = state
			.minio_client
			.put_object(&state.bucket_name, &file.filename, SegmentedBytes::from(file.bytes))
			.send()
			.await;

		let client = state.pool.get().await.unwrap();
		let location = if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
			Some(Location { latitude, longitude })
		} else {
			None
		};

		let _ = Mutation::upsert_s3_object_impl(&client, file.filename, None, location).await;
	}

	StatusCode::OK
}
