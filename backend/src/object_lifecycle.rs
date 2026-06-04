//! Object storage lifecycle: upload sessions, completion and abort, the
//! storage-deletion outbox, and the periodic maintenance worker.
//!
//! Split by altitude across submodules: `config` holds the policy knobs and
//! part math, `service` the request-time operations and maintenance routines,
//! `worker` the periodic driver, and `deletion` the storage-deletion outbox
//! queue and processor.

mod config;
mod deletion;
mod service;
mod worker;

pub use {
	config::ObjectLifecycleConfig,
	service::{
		CreatedObjectUploadSession,
		ObjectLifecycleService,
		ObjectUploadSessionCreate,
		PresignedObjectUploadPart,
	},
	worker::ObjectLifecycleWorker,
};
