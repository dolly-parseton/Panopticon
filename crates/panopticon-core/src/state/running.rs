use crate::imports::*;

impl Pipeline<Running> {
    /// Returns a snapshot of the worker thread's current [`RunningStatus`]
    /// without blocking. A `Completed`, `Cancelled`, or `Failed` result
    /// means a subsequent [`wait`](Self::wait) will return immediately.
    pub fn poll(&self) -> RunningStatus {
        match self.state.result.lock().unwrap().as_ref() {
            Some(Ok(_)) => RunningStatus::Completed, // Completed successfully
            Some(Err(OperationError::Cancelled)) => RunningStatus::Cancelled, // Cancelled (treated as a special error case)
            Some(Err(_)) => RunningStatus::Failed, // Completed with error
            None => RunningStatus::Running,        // Still running
        }
    }

    /// Joins the worker thread and promotes the pipeline from [`Running`]
    /// to [`Complete`].
    ///
    /// Blocks until the worker thread exits. Returns the [`OperationError`]
    /// produced by the worker if execution failed, or a panic-mapped
    /// [`OperationError::ThreadPanic`] if the worker thread panicked.
    pub fn wait(self) -> Result<Pipeline<Complete>, OperationError> {
        self.state
            .handle
            .join()
            .map_err(|_| OperationError::ThreadPanic)?;
        match self.state.result.lock().unwrap().take() {
            Some(Ok(complete)) => Ok(Pipeline { state: complete }),
            Some(Err(e)) => Err(e),
            None => unreachable!("Thread should have set the result before exiting"),
        }
    }

    /// Signals the worker thread to stop at the next step boundary.
    ///
    /// Consumes the pipeline and does not wait for the thread to exit. The
    /// cancellation is cooperative — it takes effect only between steps,
    /// so an in-flight operation runs to completion before observing it.
    pub fn cancel(self) {
        self.state
            .cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
