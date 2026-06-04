pub const ALLOWED_MIME_TYPES: [&str; 17] = [
	"image/png",
	"image/jpeg",
	"image/gif",
	"image/webp",
	"image/svg+xml",
	"image/avif",
	"image/apng",
	"video/mp4",
	"video/webm",
	"video/ogg",
	"audio/mpeg",
	"audio/wav",
	"audio/ogg",
	"audio/webm",
	"audio/flac",
	"audio/aac",
	"audio/mp4",
];

/// Maximum number of upload parts the backend will presign in one request, and
/// the chunk size the frontend uses when batching presign requests. The two
/// MUST agree, so this constant is the single source of truth shared across the
/// API boundary.
pub const MAX_PRESIGN_PARTS_PER_REQUEST: usize = 100;
