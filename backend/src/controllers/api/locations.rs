use {
	crate::{
		AppState,
		constants::{
			ERR_DB_CLIENT,
			ERR_FAILED_READ_BYTES,
			ERR_MULTIPART_MISSING_CONTENT_TYPE,
			ERR_MULTIPART_MISSING_FILENAME,
			ERR_MULTIPART_MISSING_NAME,
			ERR_UNSUPPORTED_FILE_TYPE,
		},
		errors::AppError,
		graphql::{
			objects::{
				location::Location,
				s3_object::PublicityOverride,
			},
			queries::mutation::Mutation,
		},
	},
	anyhow::Context,
	axum::{
		Json,
		body::Bytes,
		extract::{
			Multipart,
			State,
		},
		response::IntoResponse,
	},
	axum_extra::extract::cookie::PrivateCookieJar,
	axum_macros::debug_handler,
	deadpool::managed::Object,
	deadpool_postgres::Manager,
	shared::ALLOWED_MIME_TYPES,
};

#[derive(Debug)]
struct FileData {
	filename: String,
	content_type: String,
	bytes: Bytes,
}

fn parse_coordinate(
	name: &str,
	value: String,
) -> Result<f64, AppError> {
	value.parse::<f64>().map_err(|e| AppError::Validation(format!("{name} must be a number: {e}")))
}

#[debug_handler]
pub async fn post(
	State(state): State<AppState<Manager, Object<Manager>>>,
	jar: PrivateCookieJar,
	mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
	let user_id = if let Some(cookie) = jar.get("auth_token") &&
		let Ok(id) = cookie.value().parse::<i64>()
	{
		id
	} else {
		return Err(AppError::Unauthorized);
	};

	let mut latitude: Option<f64> = None;
	let mut longitude: Option<f64> = None;
	let mut made_on: Option<String> = None;
	let mut files: Vec<FileData> = Vec::new();

	while let Some(field) = multipart.next_field().await? {
		let name = field
			.name()
			.ok_or_else(|| AppError::Validation(ERR_MULTIPART_MISSING_NAME.to_string()))?
			.to_string();

		match name.as_str() {
			"latitude" => {
				latitude = Some(parse_coordinate("latitude", field.text().await?)?);
			}
			"longitude" => {
				longitude = Some(parse_coordinate("longitude", field.text().await?)?);
			}
			"made_on" => {
				if let Ok(txt) = field.text().await &&
					!txt.is_empty()
				{
					// Store the ISO 8601 UTC timestamp string
					made_on = Some(txt);
				}
			}
			"files" => {
				let filename = field
					.file_name()
					.ok_or_else(|| {
						AppError::Validation(ERR_MULTIPART_MISSING_FILENAME.to_string())
					})?
					.to_string();
				let content_type = field
					.content_type()
					.ok_or_else(|| {
						AppError::Validation(ERR_MULTIPART_MISSING_CONTENT_TYPE.to_string())
					})?
					.to_string();

				if !ALLOWED_MIME_TYPES.contains(&content_type.as_str()) {
					return Err(AppError::Validation(format!(
						"{}{}",
						ERR_UNSUPPORTED_FILE_TYPE, content_type
					)));
				}

				let bytes = field
					.bytes()
					.await
					.map_err(|e| AppError::Validation(format!("{}{}", ERR_FAILED_READ_BYTES, e)))?;
				files.push(FileData {
					filename,
					content_type,
					bytes,
				});
			}
			_ => {}
		}
	}

	tracing::debug!("Received Location:");
	tracing::debug!("Latitude: {:?}", latitude);
	tracing::debug!("Longitude: {:?}", longitude);
	tracing::debug!("Files: {} uploaded", files.len());

	let mut uploaded_objects = Vec::new();
	let location = if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
		Some(
			Location {
				latitude,
				longitude,
			}
			.validated()?,
		)
	} else {
		None
	};

	for file in files {
		let FileData {
			filename,
			content_type,
			bytes,
		} = file;
		tracing::debug!(
			" - Name: {}, Type: {}, Size: {} bytes",
			filename,
			content_type,
			bytes.len()
		);

		state.inner.storage.upload_object(&filename, bytes, content_type).await?;

		let client = state.inner.pool.get().await.context(ERR_DB_CLIENT)?;

		match Mutation::upsert_s3_object_worker(
			&client,
			filename,
			made_on.clone(),
			location.clone(),
			user_id,
			PublicityOverride::Default,
			vec![],
		)
		.await
		{
			Ok(s3_object) => uploaded_objects.push(s3_object),
			Err(e) => {
				tracing::error!("Failed to upsert object: {:?}", e);
				return Err(e);
			}
		}

		state.inner.update_last_modified();
	}

	Ok(Json(uploaded_objects).into_response())
}
