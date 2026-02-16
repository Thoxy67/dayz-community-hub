use crate::api::Server;
use crate::config::LaunchOptions;

/// Build DayZ launch arguments for connecting to a server.
///
/// Produces arguments like:
///   `-mod=@id1;@id2 -connect=IP -port=PORT -password=PASS -nosplash ...`
pub fn build_launch_args(
    server: &Server,
    mod_ids: &[u64],
    password: Option<&str>,
    launch_options: &LaunchOptions,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();

    // Mods: -mod=@id1;@id2;@id3
    if !mod_ids.is_empty() {
        let mods_str = mod_ids
            .iter()
            .map(|id| format!("@{}", id))
            .collect::<Vec<_>>()
            .join(";");
        args.push(format!("-mod={}", mods_str));
    }

    // Connection
    args.push(format!("-connect={}", server.endpoint.ip));
    args.push(format!("-port={}", server.game_port));

    // Password
    if let Some(pass) = password {
        if !pass.is_empty() {
            args.push(format!("-password={}", pass));
        }
    }

    // Profile launch options (nosplash, skipintro, high, etc.)
    args.extend(launch_options.to_args());

    // Extra arguments
    args.extend(extra_args.iter().cloned());

    args
}

/// Wrap DayZ args with Steam's `-applaunch` prefix.
///
/// Produces:
///   `steam -applaunch 221100 -nolauncher -name=PLAYER <dayz_args>`
pub fn build_steam_applaunch_args(
    game_id: u32,
    args: &[String],
    username: Option<&str>,
) -> Vec<String> {
    let mut steam_args = vec![
        "-applaunch".to_string(),
        game_id.to_string(),
        "-nolauncher".to_string(),
    ];
    if let Some(user) = username {
        if !user.is_empty() {
            steam_args.push(format!("-name={}", user));
        }
    }
    steam_args.extend(args.iter().cloned());
    steam_args
}
