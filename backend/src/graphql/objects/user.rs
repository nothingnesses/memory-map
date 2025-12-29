use async_graphql::{Enum, Object, ID};
use jiff::Timestamp;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum UserRole {
	User,
	Admin,
}

impl ToString for UserRole {
	fn to_string(&self) -> String {
		match self {
			UserRole::User => "user".to_string(),
			UserRole::Admin => "admin".to_string(),
		}
	}
}

impl std::str::FromStr for UserRole {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"user" => Ok(UserRole::User),
			"admin" => Ok(UserRole::Admin),
			_ => Err(()),
		}
	}
}

#[derive(Clone, Debug)]
pub struct User {
	pub id: ID,
	pub email: String,
	pub role: UserRole,
	pub created_at: Timestamp,
	pub updated_at: Timestamp,
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

	async fn created_at(&self) -> String {
		self.created_at.to_string()
	}

	async fn updated_at(&self) -> String {
		self.updated_at.to_string()
	}
}
