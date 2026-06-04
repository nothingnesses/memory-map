use {
	crate::{
		object_lifecycle::{
			CreatedObjectUploadSession as CreatedObjectUploadSessionInner,
			PresignedObjectUploadPart as PresignedObjectUploadPartInner,
		},
		storage::PresignedHeader as PresignedHeaderInner,
	},
	async_graphql::{
		ID,
		Object,
	},
	jiff::Timestamp,
};

#[derive(Clone, Debug)]
pub struct CreatedObjectUploadSession(pub CreatedObjectUploadSessionInner);

#[Object]
impl CreatedObjectUploadSession {
	async fn object_id(&self) -> ID {
		self.0.object_id.into()
	}

	async fn part_size_bytes(&self) -> i64 {
		self.0.part_size_bytes
	}

	async fn total_parts(&self) -> i32 {
		self.0.total_parts
	}

	async fn expires_at(&self) -> String {
		self.0.expires_at.to_string()
	}
}

impl From<CreatedObjectUploadSessionInner> for CreatedObjectUploadSession {
	fn from(session: CreatedObjectUploadSessionInner) -> Self {
		Self(session)
	}
}

#[derive(Clone, Copy, Debug)]
pub struct AbortedObjectUpload {
	object_id: i64,
}

impl AbortedObjectUpload {
	pub fn new(object_id: i64) -> Self {
		Self {
			object_id,
		}
	}
}

#[Object]
impl AbortedObjectUpload {
	async fn object_id(&self) -> ID {
		self.object_id.into()
	}
}

#[derive(Clone, Debug)]
pub struct PresignedObjectUploadPart {
	inner: PresignedObjectUploadPartInner,
	url_expires_at: Timestamp,
}

impl PresignedObjectUploadPart {
	pub fn new(
		inner: PresignedObjectUploadPartInner,
		url_expires_at: Timestamp,
	) -> Self {
		Self {
			inner,
			url_expires_at,
		}
	}
}

#[Object]
impl PresignedObjectUploadPart {
	async fn part_number(&self) -> i32 {
		self.inner.part_number
	}

	async fn url(&self) -> &str {
		&self.inner.url
	}

	async fn method(&self) -> &str {
		&self.inner.method
	}

	async fn headers(&self) -> Vec<PresignedRequestHeader> {
		self.inner.headers.iter().cloned().map(PresignedRequestHeader::from).collect()
	}

	async fn expected_content_length(&self) -> i64 {
		self.inner.expected_content_length
	}

	async fn url_expires_at(&self) -> String {
		self.url_expires_at.to_string()
	}
}

#[derive(Clone, Debug)]
pub struct PresignedRequestHeader(PresignedHeaderInner);

impl From<PresignedHeaderInner> for PresignedRequestHeader {
	fn from(header: PresignedHeaderInner) -> Self {
		Self(header)
	}
}

#[Object]
impl PresignedRequestHeader {
	async fn name(&self) -> &str {
		&self.0.name
	}

	async fn value(&self) -> &str {
		&self.0.value
	}
}
