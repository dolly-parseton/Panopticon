use crate::imports::*;

// TODO Running Error type
impl Pipeline<Running> {
    // State machine functions
    pub fn poll(&self) -> RunningStatus {
        match self.state.result.lock().unwrap().as_ref() {
            Some(Ok(_)) => RunningStatus::Completed, // Completed successfully
            Some(Err(OperationError::Cancelled)) => RunningStatus::Cancelled, // Cancelled (treated as a special error case)
            Some(Err(_)) => RunningStatus::Failed, // Completed with error
            None => RunningStatus::Running,        // Still running
        }
    }

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

    pub fn cancel(self) {
        self.state
            .cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
