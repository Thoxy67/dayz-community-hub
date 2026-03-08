/// Macro to handle the common "lock → mutate → save" pattern for profile operations.
/// Reduces 9+ repetitive patterns across command files.
///
/// # Examples
///
/// ```rust
/// // Before:
/// let mut state = state.lock().await;
/// state.ctl.remove_favorite(&ip, port);
/// state.ctl.save_profile().map_err(|e| e.to_string())
///
/// // After:
/// with_profile_save!(state, |ctl| {
///     ctl.remove_favorite(&ip, port);
/// })
/// ```
///
/// For simple single-line mutations:
/// ```rust
/// // Before:
/// let mut state = state.lock().await;
/// state.ctl.add_favorite(name, ip, port, password);
/// state.ctl.save_profile().map_err(|e| e.to_string())
///
/// // After:
/// with_profile_save!(state, |ctl| ctl.add_favorite(name, ip, port, password))
/// ```
#[macro_export]
macro_rules! with_profile_save {
    ($state:expr, |$ctl:ident| $mutation:expr) => {{
        use $crate::utils::error::ResultExt;
        let mut state = $state.write().await;
        $mutation(&mut state.ctl);
        state.ctl.save_profile().cmd_err()
    }};
}

/// Alternative syntax for direct method calls
#[macro_export]
macro_rules! profile_mutate_save {
    ($state:expr, $method:ident $(, $arg:expr)*) => {{
        use $crate::utils::error::ResultExt;
        let mut state = $state.write().await;
        state.ctl.$method($($arg),*);
        state.ctl.save_profile().cmd_err()
    }};
}
