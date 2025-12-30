use crate::{ContextWrapper, SharedState, graphql::objects::location::Location};
use async_graphql::{Context, Error as GraphQLError, ID, Object};
use axum::http::Method;
use deadpool_postgres::Manager;
use futures::future::join_all;
use jiff::Timestamp;
use minio::s3::types::S3Api;
use std::sync::Arc;
use tokio_postgres::Row;

#[derive(Debug)]
pub struct S3Object {
	pub id: ID,
	pub name: String,
	pub made_on: Option<Timestamp>,
	pub location: Option<Location>,
}

impl S3Object {
	pub async fn try_from(row: Row) -> Result<Self, GraphQLError> {
		let name: String = row.try_get("name")?;
		Ok(S3Object {
			id: Row::try_get::<_, i64>(&row, "id")?.into(),
			name: name.clone(),
			made_on: row.try_get("made_on")?,
			location: Location::try_from(row).ok(),
		})
	}

	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM objects;",
			)
			.await?;
		join_all(
			client
				.query(&statement, &[])
				.await
				.map_err(GraphQLError::from)?
				.into_iter()
				.map(Self::try_from)
				.collect::<Vec<_>>(),
		)
		.await
		.into_iter()
		.collect::<Result<Vec<_>, _>>()
	}

	pub async fn where_id(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM objects
				WHERE id = $1;",
			)
			.await?;
		Self::try_from(client.query_one(&statement, &[&id]).await?).await
	}

	pub async fn where_name(
		ctx: &Context<'_>,
		name: String,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, name, made_on, ST_Y(location::geometry) AS latitude, ST_X(location::geometry) AS longitude
				FROM objects
				WHERE name = $1;",
			)
			.await?;
		Self::try_from(client.query_one(&statement, &[&name]).await?).await
	}
}

#[Object]
impl S3Object {
	async fn id(&self) -> ID {
		self.id.clone()
	}

	async fn name(&self) -> String {
		self.name.to_string()
	}

	async fn made_on(&self) -> Option<String> {
		self.made_on.map(|made_on| made_on.to_string())
	}

	async fn location(&self) -> Option<Location> {
		self.location.clone()
	}

	async fn url(
		&self,
		ctx: &Context<'_>,
	) -> Result<String, GraphQLError> {
		let data = ctx.data::<Arc<SharedState<Manager, deadpool_postgres::Client>>>()?;
		Ok(data
			.minio_client
			.get_presigned_object_url(&data.bucket_name, &self.name, Method::GET)
			.send()
			.await?
			.url)
	}

	async fn content_type(
		&self,
		ctx: &Context<'_>,
	) -> Result<String, GraphQLError> {
		let data = ctx.data::<Arc<SharedState<Manager, deadpool_postgres::Client>>>()?;
		data.minio_client
			.get_object(&data.bucket_name, &self.name)
			.send()
			.await?
			.headers
			.get("Content-Type")
			.and_then(|content_type| content_type.to_str().ok())
			.map(|s| s.to_string())
			.ok_or_else(|| "Invalid Content-Type".into())
	}
}
