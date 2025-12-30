use crate::{ContextWrapper, SharedState, graphql::objects::location::Location};
use async_graphql::{Context, Enum, Error as GraphQLError, ID, Object};
use axum::http::Method;
use deadpool_postgres::Manager;
use futures::future::join_all;
use jiff::Timestamp;
use minio::s3::types::S3Api;
use std::{fmt, str::FromStr, sync::Arc};
use tokio_postgres::Row;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum PublicityOverride {
	Default,
	Public,
	Private,
	SelectedUsers,
}

impl fmt::Display for PublicityOverride {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		match self {
			PublicityOverride::Default => write!(f, "default"),
			PublicityOverride::Public => write!(f, "public"),
			PublicityOverride::Private => write!(f, "private"),
			PublicityOverride::SelectedUsers => write!(f, "selected_users"),
		}
	}
}

impl FromStr for PublicityOverride {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"default" => Ok(PublicityOverride::Default),
			"public" => Ok(PublicityOverride::Public),
			"private" => Ok(PublicityOverride::Private),
			"selected_users" => Ok(PublicityOverride::SelectedUsers),
			_ => Err(()),
		}
	}
}

#[derive(Debug)]
pub struct S3Object {
	pub id: ID,
	pub name: String,
	pub made_on: Option<Timestamp>,
	pub location: Option<Location>,
	pub user_id: Option<i64>,
	pub publicity: PublicityOverride,
	pub allowed_users: Vec<String>,
}

impl S3Object {
	pub async fn try_from(row: Row) -> Result<Self, GraphQLError> {
		let name: String = row.try_get("name")?;
		let id: i64 = row.try_get("id")?;
		let made_on: Option<Timestamp> = row.try_get("made_on")?;
		let user_id: Option<i64> = row.try_get("user_id").ok();
		let publicity_str: String = row.try_get("publicity")?;
		let publicity =
			publicity_str.parse().map_err(|_| GraphQLError::new("Invalid publicity"))?;
		// allowed_users might be null if no join, but we will ensure join
		let allowed_users: Vec<String> = row.try_get("allowed_users").unwrap_or_default();

		Ok(S3Object {
			id: id.into(),
			name,
			made_on,
			location: Location::try_from(row).ok(),
			user_id,
			publicity,
			allowed_users,
		})
	}

	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
				COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
				FROM objects o
				LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
				LEFT JOIN users u ON oau.user_id = u.id
				GROUP BY o.id;",
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
				"SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
				COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
				FROM objects o
				LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
				LEFT JOIN users u ON oau.user_id = u.id
				WHERE o.id = $1
				GROUP BY o.id;",
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
				"SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
				COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
				FROM objects o
				LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
				LEFT JOIN users u ON oau.user_id = u.id
				WHERE o.name = $1
				GROUP BY o.id;",
			)
			.await?;
		Self::try_from(client.query_one(&statement, &[&name]).await?).await
	}

	pub async fn where_ids(
		ctx: &Context<'_>,
		ids: &[i64],
	) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
				COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
				FROM objects o
				LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
				LEFT JOIN users u ON oau.user_id = u.id
				WHERE o.id = ANY($1)
				GROUP BY o.id;",
			)
			.await?;
		join_all(
			client
				.query(&statement, &[&ids])
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

	pub async fn where_user_id(
		ctx: &Context<'_>,
		user_id: i64,
	) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
				COALESCE(array_agg(u.email) FILTER (WHERE u.email IS NOT NULL), '{}') AS allowed_users
				FROM objects o
				LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
				LEFT JOIN users u ON oau.user_id = u.id
				WHERE o.user_id = $1
				GROUP BY o.id;",
			)
			.await?;
		join_all(
			client
				.query(&statement, &[&user_id])
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

	pub async fn visible_to_user(
		ctx: &Context<'_>,
		user_id: Option<i64>,
	) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT o.id, o.name, o.made_on, ST_Y(o.location::geometry) AS latitude, ST_X(o.location::geometry) AS longitude, o.user_id, o.publicity,
				COALESCE(array_agg(u_allowed.email) FILTER (WHERE u_allowed.email IS NOT NULL), '{}') AS allowed_users
				FROM objects o
				JOIN users u ON o.user_id = u.id
				LEFT JOIN object_allowed_users oau ON o.id = oau.object_id
				LEFT JOIN users u_allowed ON oau.user_id = u_allowed.id
				WHERE
					($1::BIGINT IS NOT NULL AND o.user_id = $1)
					OR o.publicity = 'public'
					OR (o.publicity = 'default' AND u.default_publicity = 'public')
					OR (o.publicity = 'selected_users' AND $1::BIGINT IS NOT NULL AND $1 IN (SELECT user_id FROM object_allowed_users WHERE object_id = o.id))
				GROUP BY o.id;",
			)
			.await?;
		join_all(
			client
				.query(&statement, &[&user_id])
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

	async fn publicity(&self) -> PublicityOverride {
		self.publicity
	}

	async fn allowed_users(&self) -> Vec<String> {
		self.allowed_users.clone()
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
