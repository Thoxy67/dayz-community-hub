/// Extension trait to convert Result errors to String for Tauri commands.
/// Eliminates 85+ `.map_err(|e| e.to_string())` occurrences across the codebase.
///
/// # Usage
/// ```ignore
/// use crate::utils::error::ResultExt;
///
/// // Before:
/// some_result.map_err(|e| e.to_string())?;
///
/// // After:
/// some_result.cmd_err()?;
/// ```
pub trait ResultExt<T> {
    /// Convert the error to a String for Tauri command compatibility.
    fn cmd_err(self) -> Result<T, String>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for Result<T, E> {
    fn cmd_err(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}
