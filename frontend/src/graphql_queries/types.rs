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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
	#[serde(rename = "USER")]
	User,
	#[serde(rename = "ADMIN")]
	Admin,
}
