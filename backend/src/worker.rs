use {
	crate::errors::AppError,
	std::{
		future::Future,
		time::Duration,
	},
	tokio::{
		task::JoinHandle,
		time::{
			MissedTickBehavior,
			interval,
		},
	},
};

/// A unit of periodic background work driven by [`spawn`].
///
/// `run_once` is declared with an explicit `Send` bound on its returned future
/// (rather than as a bare `async fn`) because [`spawn`] runs it inside a
/// `tokio::spawn`ed task, which requires the future to be `Send`. A plain
/// async-fn-in-trait cannot express that bound at the call site. Implementors
/// may still write the method body as an ordinary `async fn`.
pub trait MaintenanceTask: Send + Sync + 'static {
	/// Stable identifier used as log context when a pass fails.
	fn name(&self) -> &'static str;

	/// Delay between maintenance passes.
	fn interval(&self) -> Duration;

	/// Runs a single maintenance pass. Errors are logged by the driver and do
	/// not stop the loop.
	fn run_once(&self) -> impl Future<Output = Result<(), AppError>> + Send;
}

/// Spawns `task` on a periodic loop using missed-tick delay semantics, logging
/// (but not propagating) any per-pass error so a transient failure does not
/// stop future passes.
pub fn spawn<T: MaintenanceTask>(task: T) -> JoinHandle<()> {
	tokio::spawn(async move {
		let mut ticker = interval(task.interval());
		ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

		loop {
			ticker.tick().await;
			if let Err(error) = task.run_once().await {
				tracing::warn!(
					error = ?error,
					task = task.name(),
					"Background maintenance task failed"
				);
			}
		}
	})
}
