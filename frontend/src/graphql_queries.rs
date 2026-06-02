use {
	crate::{
		errors::{
			AppError,
			graphql_data,
		},
		post_graphql,
		post_graphql_with_auth,
	},
	graphql_client::GraphQLQuery,
};

pub mod abort_object_upload;
pub mod admin_update_user;
pub mod change_email;
pub mod change_password;
pub mod complete_object_upload;
pub mod config;
pub mod create_object_upload_session;
pub mod delete_s3_objects;
pub mod login;
pub mod logout;
pub mod me;
pub mod presign_object_upload_parts;
pub mod register;
pub mod request_password_reset;
pub mod reset_password;
pub mod s3_object_by_id;
pub mod s3_objects;
pub mod types;
pub mod update_s3_object;
pub mod update_user_publicity;
pub mod users;

pub trait GraphqlOp: GraphQLQuery {
	type Output;

	fn extract(data: Self::ResponseData) -> Self::Output;
}

pub async fn run<O: GraphqlOp>(
	api_url: String,
	variables: O::Variables,
) -> Result<O::Output, AppError> {
	let response =
		post_graphql_with_auth::<O, _>(&reqwest::Client::new(), api_url, variables).await?;
	Ok(O::extract(graphql_data(response)?))
}

pub async fn run_unauthenticated<O: GraphqlOp>(
	api_url: String,
	variables: O::Variables,
) -> Result<O::Output, AppError> {
	let response = post_graphql::<O, _>(&reqwest::Client::new(), api_url, variables).await?;
	Ok(O::extract(graphql_data(response)?))
}
