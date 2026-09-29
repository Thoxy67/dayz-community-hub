//! Errors as the window receives them: a message.

/// Turn any displayable error into the `String` a command rejects with.
pub trait ResultExt<T> {
    fn cmd_err(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for Result<T, E> {
    fn cmd_err(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

/// Run blocking work on the blocking pool and flatten the join error and the
/// work's own error into one message.
pub(crate) async fn spawn_blocking_mapped<T, E>(
    f: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
    E: ToString + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| format!("Task join error: {e}"))?
        .map_err(|e| e.to_string())
}
