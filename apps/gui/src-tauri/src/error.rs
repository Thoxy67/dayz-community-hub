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

/// An HTTP failure as a message, without the request's URL: URLs carry API
/// keys (Steam, BattleMetrics) that must not reach a toast or a log.
pub(crate) trait HttpResultExt<T> {
    fn http_err(self, what: &str) -> Result<T, String>;
}

impl<T> HttpResultExt<T> for Result<T, reqwest::Error> {
    fn http_err(self, what: &str) -> Result<T, String> {
        self.map_err(|e| {
            let e = e.without_url();
            if e.is_timeout() {
                format!("{what}: the server did not answer in time")
            } else if e.is_connect() {
                format!("{what}: could not connect ({e})")
            } else if let Some(status) = e.status() {
                format!("{what}: HTTP {status}")
            } else {
                format!("{what}: {e}")
            }
        })
    }
}

/// Send a request and fail on a non-2xx status, as one step.
pub(crate) async fn send_ok(
    req: reqwest::RequestBuilder,
) -> Result<reqwest::Response, reqwest::Error> {
    req.send().await?.error_for_status()
}
