use {
	super::{
		ObjectLifecycleConfig,
		ObjectLifecycleService,
	},
	crate::{
		errors::AppError,
		storage::StorageClient,
		worker::MaintenanceTask,
	},
	deadpool::managed::Pool,
	deadpool_postgres::Manager,
	std::time::Duration,
};

#[derive(Clone)]
pub struct ObjectLifecycleWorker {
	pool: Pool<Manager>,
	storage: StorageClient,
	config: ObjectLifecycleConfig,
}

impl ObjectLifecycleWorker {
	pub fn new(
		pool: Pool<Manager>,
		storage: StorageClient,
		config: ObjectLifecycleConfig,
	) -> Self {
		Self {
			pool,
			storage,
			config,
		}
	}
}

impl MaintenanceTask for ObjectLifecycleWorker {
	fn name(&self) -> &'static str {
		"object_storage_lifecycle"
	}

	fn interval(&self) -> Duration {
		self.config.worker_interval()
	}

	async fn run_once(&self) -> Result<(), AppError> {
		let mut client = self.pool.get().await?;
		let mut object_lifecycle =
			ObjectLifecycleService::new(&mut client, &self.storage, self.config.clone());
		object_lifecycle.run_storage_maintenance().await
	}
}
