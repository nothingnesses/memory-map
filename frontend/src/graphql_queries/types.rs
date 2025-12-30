use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublicityOverride {
	#[serde(rename = "DEFAULT")]
	Default,
	#[serde(rename = "PUBLIC")]
	Public,
	#[serde(rename = "PRIVATE")]
	Private,
	#[serde(rename = "SELECTED_USERS")]
	SelectedUsers,
}

impl fmt::Display for PublicityOverride {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		match self {
			PublicityOverride::Default => write!(f, "Default"),
			PublicityOverride::Public => write!(f, "Public"),
			PublicityOverride::Private => write!(f, "Private"),
			PublicityOverride::SelectedUsers => write!(f, "Selected Users"),
		}
	}
}

impl std::str::FromStr for PublicityOverride {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Default" => Ok(PublicityOverride::Default),
			"Public" => Ok(PublicityOverride::Public),
			"Private" => Ok(PublicityOverride::Private),
			"Selected Users" => Ok(PublicityOverride::SelectedUsers),
			_ => Err(()),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublicityDefault {
	#[serde(rename = "PUBLIC")]
	Public,
	#[serde(rename = "PRIVATE")]
	Private,
}

impl fmt::Display for PublicityDefault {
	fn fmt(
		&self,
		f: &mut fmt::Formatter<'_>,
	) -> fmt::Result {
		match self {
			PublicityDefault::Public => write!(f, "Public"),
			PublicityDefault::Private => write!(f, "Private"),
		}
	}
}

impl std::str::FromStr for PublicityDefault {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Public" => Ok(PublicityDefault::Public),
			"Private" => Ok(PublicityDefault::Private),
			_ => Err(()),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
	#[serde(rename = "USER")]
	User,
	#[serde(rename = "ADMIN")]
	Admin,
}
