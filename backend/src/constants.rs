// Total bytes the GraphQL response cache may hold across all entries, enforced
// through a per-entry weigher. Keeps memory predictable independent of how many
// distinct query/user pairs are cached.
pub const GRAPHQL_RESPONSE_CACHE_MAX_CAPACITY_BYTES: u64 = 64 * 1024 * 1024;
// Cache time-to-live duration in seconds. A short TTL is a backstop for changes
// made outside this process; in-process writes invalidate the cache explicitly.
pub const GRAPHQL_RESPONSE_CACHE_TTL_SECONDS: u64 = 600;
// Max body size for GraphQL requests (1MB).
pub const GRAPHQL_BODY_LIMIT_BYTES: usize = 1024 * 1024;

// 1GB.
pub const BODY_MAX_SIZE_LIMIT_BYTES: usize = 1_073_741_824;

// Minimum seconds between password reset token issuances per user.
// Prevents bursts of password-reset emails to a single account, without
// causing user enumeration: throttled requests still return success.
pub const PASSWORD_RESET_RATE_LIMIT_SECONDS: i64 = 60;

// Error messages still used as bare validation strings or REST response bodies.
// Per-source-type prefixes (e.g. "Database error: ") were removed when the
// AppError From impls were funneled through anyhow; site-specific context lives
// in `.context("Failed to ...")` chains rather than per-type constants.
pub const ERR_MULTIPART_MISSING_NAME: &str = "Multipart field missing name";
pub const ERR_MULTIPART_MISSING_FILENAME: &str = "Multipart field missing filename";
pub const ERR_MULTIPART_MISSING_CONTENT_TYPE: &str = "Multipart field missing content type";
pub const ERR_UNSUPPORTED_FILE_TYPE: &str = "Unsupported file type: ";
pub const ERR_FAILED_READ_BYTES: &str = "Failed to read bytes: ";
pub const ERR_UPLOAD_STORAGE: &str = "Failed to upload file to S3 storage";
pub const ERR_DB_CLIENT: &str = "Failed to get database client from pool";
