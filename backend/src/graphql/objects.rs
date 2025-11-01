use async_graphql::Context;
use tokio_postgres::Row;

pub mod location;
pub mod s3object;

pub struct RowContext<'a>(pub Row, pub Context<'a>);
