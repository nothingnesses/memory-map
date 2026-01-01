// Amount of bytes to cache.
pub const CACHE_MAX_CAPACITY: u64 = 10_000;
// Cache time-to-live duration in seconds. Currently 10 minutes.
pub const CACHE_TTL_SECONDS: u64 = 600;
// Max body size for GraphQL queries (1MB).
pub const GRAPHQL_BODY_LIMIT_BYTES: usize = 1024 * 1024;

// 1GB.
pub const BODY_MAX_SIZE_LIMIT_BYTES: usize = 1_073_741_824;
