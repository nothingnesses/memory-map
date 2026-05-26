// Amount of bytes to cache.
pub const CACHE_MAX_CAPACITY: u64 = 10_000;
// Cache time-to-live duration in seconds. Currently 10 minutes.
pub const CACHE_TTL_SECONDS: u64 = 600;
// Max body size for GraphQL queries (1MB).
pub const GRAPHQL_BODY_LIMIT_BYTES: usize = 1024 * 1024;

// 1GB.
pub const BODY_MAX_SIZE_LIMIT_BYTES: usize = 1_073_741_824;

// Errors
pub const ERR_INTERNAL_SERVER: &str = "Internal server error";
pub const ERR_UNAUTHORIZED: &str = "Unauthorized";
pub const ERR_FORBIDDEN: &str = "Forbidden";
pub const ERR_NOT_FOUND: &str = "Not found: ";
pub const ERR_VALIDATION: &str = "Validation error: ";
pub const ERR_MULTIPART_MISSING_NAME: &str = "Multipart field missing name";
pub const ERR_MULTIPART_MISSING_FILENAME: &str = "Multipart field missing filename";
pub const ERR_MULTIPART_MISSING_CONTENT_TYPE: &str = "Multipart field missing content type";
pub const ERR_UNSUPPORTED_FILE_TYPE: &str = "Unsupported file type: ";
pub const ERR_FAILED_READ_BYTES: &str = "Failed to read bytes: ";
pub const ERR_UPLOAD_STORAGE: &str = "Failed to upload file to S3 storage";
pub const ERR_DB_CLIENT: &str = "Failed to get database client from pool";
pub const ERR_HASHING: &str = "Hashing error: ";
pub const ERR_EMAIL: &str = "Email error: ";
pub const ERR_EMAIL_ADDRESS: &str = "Email address error: ";
pub const ERR_SMTP: &str = "SMTP error: ";
pub const ERR_INVALID_NUMBER: &str = "Invalid number format: ";
pub const ERR_CREATE_POOL: &str = "Failed to create pool: ";
pub const ERR_POOL: &str = "Pool error: ";
pub const ERR_DB: &str = "Database error: ";
pub const ERR_MULTIPART: &str = "Multipart error: ";
pub const ERR_CASBIN: &str = "Casbin error: ";
pub const ERR_CONFIG: &str = "Config error: ";
pub const ERR_MIGRATION: &str = "Migration error: ";
pub const ERR_SYSTEM_TIME: &str = "System time error: ";
pub const ERR_IO: &str = "IO error: ";
