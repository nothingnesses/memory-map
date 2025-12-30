use crate::ContextWrapper;
use async_graphql::{Context, Enum, Error as GraphQLError, ID, Object};
use jiff::Timestamp;
use postgres_types::{FromSql, ToSql};
use std::{fmt, str::FromStr};
use tokio_postgres::Row;

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

impl User {
	pub fn try_from(row: Row) -> Result<Self, GraphQLError> {
		let role_str: String = row.try_get("role")?;
		let role = role_str.parse().map_err(|_| GraphQLError::new("Invalid role"))?;
		let default_publicity: PublicityDefault = row.try_get("default_publicity")?;
		Ok(User {
			id: Row::try_get::<_, i64>(&row, "id")?.into(),
			email: row.try_get("email")?,
			role,
			default_publicity,
			created_at: row.try_get("created_at")?,
			updated_at: row.try_get("updated_at")?,
		})
	}

	pub async fn all(ctx: &Context<'_>) -> Result<Vec<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, email, role, created_at, updated_at, default_publicity FROM users",
			)
			.await?;
		client.query(&statement, &[]).await?.into_iter().map(Self::try_from).collect()
	}

	pub async fn by_id(
		ctx: &Context<'_>,
		id: i64,
	) -> Result<Option<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, email, role, created_at, updated_at, default_publicity FROM users WHERE id = $1",
			)
			.await?;
		match client.query_opt(&statement, &[&id]).await? {
			Some(row) => Ok(Some(Self::try_from(row)?)),
			None => Ok(None),
		}
	}

	pub async fn by_email(
		ctx: &Context<'_>,
		email: &str,
	) -> Result<Option<Self>, GraphQLError> {
		let client = ContextWrapper(ctx).get_db_client().await?;
		let statement = client
			.prepare_cached(
				"SELECT id, email, role, created_at, updated_at, default_publicity FROM users WHERE email = $1",
			)
			.await?;
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
