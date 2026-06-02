use {
	crate::{
		ContextWrapper,
		db::queries::{
			SELECT_ALL_OBJECTS_QUERY,
			SELECT_OBJECT_BY_ID_QUERY,
			SELECT_OBJECT_BY_NAME_QUERY,
			SELECT_OBJECTS_BY_IDS_QUERY,
			SELECT_OBJECTS_BY_USER_ID_QUERY,
			SELECT_VISIBLE_OBJECTS_QUERY,
		},
		errors::AppError,
		graphql::objects::location::Location,
	},
	async_graphql::{
		Context,
		Enum,
		Error as GraphQLError,
		ID,
		Object,
	},
	jiff::Timestamp,
	postgres_types::{
		FromSql,
		ToSql,
	},
	serde::{
		Serialize,
		Serializer,
	},
	std::{
		fmt,
		str::FromStr,
	},
	tokio_postgres::Row,
};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, ToSql, FromSql, Serialize)]
#[postgres(name = "publicity_override")]
pub enum PublicityOverride {
	#[postgres(name = "default")]
	Default,
	#[postgres(name = "public")]
	Public,
	#[postgres(name = "private")]
	Private,
	#[postgres(name = "selected_users")]
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

fn serialize_timestamp<S>(
	timestamp: &Option<Timestamp>,
	serializer: S,
) -> Result<S::Ok, S::Error>
where
	S: Serializer, {
	match timestamp {
		Some(ts) => serializer.serialize_str(&ts.to_string()),
		None => serializer.serialize_none(),
	}
}

/// Emits a 64-bit id as a JSON string.
///
/// Matches the GraphQL `ID` wire format and protects JavaScript clients from the
/// `Number` precision ceiling at 2^53; both endpoints (REST upload, GraphQL) now
/// agree that ids are strings.
fn serialize_i64_as_string<S>(
	value: &i64,
	serializer: S,
) -> Result<S::Ok, S::Error>
where
	S: Serializer, {
	serializer.collect_str(value)
}

#[derive(Debug, Serialize)]
pub struct S3Object {
	#[serde(serialize_with = "serialize_i64_as_string")]
	pub id: i64,
	pub name: String,
	pub storage_key: String,
	pub content_type: String,
	#[serde(serialize_with = "serialize_timestamp")]
	pub made_on: Option<Timestamp>,
	pub location: Option<Location>,
	pub user_id: Option<i64>,
	pub publicity: PublicityOverride,
	pub allowed_users: Vec<String>,
}

impl TryFrom<Row> for S3Object {
	type Error = GraphQLError;

	fn try_from(row: Row) -> Result<Self, Self::Error> {
		let name: String = row.try_get("name")?;
		let storage_key: String = row.try_get("storage_key")?;
		let content_type: String = row.try_get("content_type")?;
		let id: i64 = row.try_get("id")?;
		let made_on: Option<Timestamp> = row.try_get("made_on")?;
		let user_id: Option<i64> = row.try_get("user_id").ok();
		let publicity: PublicityOverride = row.try_get("publicity")?;
		// allowed_users might be null if no join, but we will ensure join
		let allowed_users: Vec<String> = row.try_get("allowed_users").unwrap_or_default();

		// Distinguish "no location set" (both NULL) from a decode failure.
		// ST_Y/ST_X return NULL only when `location` is NULL, so a single-NULL pair
		// signals a query shape regression and must propagate rather than be hidden.
		let latitude: Option<f64> = row.try_get("latitude")?;
		let longitude: Option<f64> = row.try_get("longitude")?;
		let location = match (latitude, longitude) {
			(Some(latitude), Some(longitude)) => Some(
				Location {
					latitude,
					longitude,
				}
				.validated()?,
			),
			(None, None) => None,
			_ =>
				return Err(GraphQLError::new(
					"Object row has only one of (latitude, longitude) set",
				)),
		};

		Ok(S3Object {
			id,
			name,
			storage_key,
			content_type,
			made_on,
			location,
			user_id,
			publicity,
			allowed_users,
		})
	}
}

impl S3Object {
	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_ALL_OBJECTS_QUERY).await?;
		client
			.query(&statement, &[])
			.await
			.map_err(AppError::graphql)?
			.into_iter()
			.map(Self::try_from)
			.collect()
	}

	pub async fn where_id(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_OBJECT_BY_ID_QUERY).await?;
		Self::try_from(client.query_one(&statement, &[&id]).await?)
	}

	pub async fn where_name(
		ctx: &Context<'_>,
		name: String,
	) -> Result<Self, GraphQLError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_OBJECT_BY_NAME_QUERY).await?;
		Self::try_from(client.query_one(&statement, &[&name]).await?)
	}

	pub async fn where_ids(
		ctx: &Context<'_>,
		ids: &[i64],
	) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_OBJECTS_BY_IDS_QUERY).await?;
		client
			.query(&statement, &[&ids])
			.await
			.map_err(AppError::graphql)?
			.into_iter()
			.map(Self::try_from)
			.collect()
	}

	pub async fn where_user_id(
		ctx: &Context<'_>,
		user_id: i64,
	) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_OBJECTS_BY_USER_ID_QUERY).await?;
		client
			.query(&statement, &[&user_id])
			.await
			.map_err(AppError::graphql)?
			.into_iter()
			.map(Self::try_from)
			.collect()
	}

	pub async fn visible_to_user(
		ctx: &Context<'_>,
		user_id: Option<i64>,
	) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_VISIBLE_OBJECTS_QUERY).await?;
		client
			.query(&statement, &[&user_id])
			.await
			.map_err(AppError::graphql)?
			.into_iter()
			.map(Self::try_from)
			.collect()
	}
}

#[Object]
impl S3Object {
	async fn id(&self) -> ID {
		self.id.into()
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
		let wrapper = ContextWrapper::new(ctx)?;
		wrapper
			.shared_state()
			.storage
			.presigned_get_url(&self.storage_key)
			.await
			.map_err(AppError::graphql)
	}

	async fn content_type(&self) -> String {
		self.content_type.clone()
	}
}
