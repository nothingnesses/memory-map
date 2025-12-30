use async_graphql::Object;

#[derive(Clone, Debug)]
pub struct PublicConfig {
	pub enable_registration: bool,
}

#[Object]
impl PublicConfig {
	async fn enable_registration(&self) -> bool {
		self.enable_registration
	}
}
