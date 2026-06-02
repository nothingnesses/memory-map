use {
	crate::{
		ContextWrapper,
		db::queries::{
			SELECT_ALL_USERS_QUERY,
			SELECT_USER_BY_EMAIL_QUERY,
			SELECT_USER_BY_ID_QUERY,
		},
		errors::AppError,
	},
	anyhow::Context as AnyhowContext,
	async_graphql::{
		Context,
		Enum,
		ID,
		Object,
	},
	jiff::Timestamp,
	postgres_types::{
		FromSql,
		ToSql,
	},
	std::{
		fmt,
		str::FromStr,
	},
	tokio_postgres::Row,
};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum UserRole {
	User,
	Admin,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, ToSql, FromSql)]
#[postgres(name = "publicity_default")]
pub enum PublicityDefault {
	#[postgres(name = "public")]
	Public,
	#[postgres(name = "private")]
	Private,
}

impl fmt::Display for UserRole {
	fn fmt(
		&self,
		f: &mut std::fmt::Formatter<'_>,
	) -> std::fmt::Result {
		match self {
			UserRole::User => write!(f, "user"),
			UserRole::Admin => write!(f, "admin"),
		}
	}
}

impl fmt::Display for PublicityDefault {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		match self {
			PublicityDefault::Public => write!(f, "public"),
			PublicityDefault::Private => write!(f, "private"),
		}
	}
}

impl FromStr for UserRole {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"user" => Ok(UserRole::User),
			"admin" => Ok(UserRole::Admin),
			_ => Err(()),
		}
	}
}

impl FromStr for PublicityDefault {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"public" => Ok(PublicityDefault::Public),
			"private" => Ok(PublicityDefault::Private),
			_ => Err(()),
		}
	}
}

#[derive(Clone, Debug)]
pub struct User {
	pub id: ID,
	pub email: String,
	pub role: UserRole,
	pub default_publicity: PublicityDefault,
	pub created_at: Timestamp,
	pub updated_at: Timestamp,
}

impl TryFrom<Row> for User {
	type Error = AppError;

	fn try_from(row: Row) -> Result<Self, Self::Error> {
		let role_str: String = row.try_get("role").context("Failed to read user role")?;
		let role = role_str
			.parse()
			.map_err(|_| AppError::Internal(anyhow::anyhow!("Invalid role in user row")))?;
		let default_publicity: PublicityDefault =
			row.try_get("default_publicity").context("Failed to read user default_publicity")?;
		Ok(User {
			id: Row::try_get::<_, i64>(&row, "id").context("Failed to read user id")?.into(),
			email: row.try_get("email").context("Failed to read user email")?,
			role,
			default_publicity,
			created_at: row.try_get("created_at").context("Failed to read user created_at")?,
			updated_at: row.try_get("updated_at").context("Failed to read user updated_at")?,
		})
	}
}

impl User {
	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, AppError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_ALL_USERS_QUERY).await?;
		client.query(&statement, &[]).await?.into_iter().map(Self::try_from).collect()
	}

	pub async fn by_id(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Option<Self>, AppError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_USER_BY_ID_QUERY).await?;
		match client.query_opt(&statement, &[&id]).await? {
			Some(row) => Ok(Some(Self::try_from(row)?)),
			None => Ok(None),
		}
	}

	pub async fn by_email(
		ctx: &Context<'_>,
		email: &str,
	) -> Result<Option<Self>, AppError> {
		let client = ContextWrapper::new(ctx)?.db_client().await?;
		let statement = client.prepare_cached(SELECT_USER_BY_EMAIL_QUERY).await?;
		match client.query_opt(&statement, &[&email]).await? {
			Some(row) => Ok(Some(Self::try_from(row)?)),
			None => Ok(None),
		}
	}
}

#[Object]
impl User {
	async fn id(&self) -> &ID {
		&self.id
	}

	async fn email(&self) -> &str {
		&self.email
	}

	async fn role(&self) -> UserRole {
		self.role
	}

	async fn default_publicity(&self) -> PublicityDefault {
		self.default_publicity
	}

	async fn created_at(&self) -> String {
		self.created_at.to_string()
	}

	async fn updated_at(&self) -> String {
		self.updated_at.to_string()
	}
}
