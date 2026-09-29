use dz_api::Server;
use dz_profile::LaunchOptions;

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
    if let Some(pass) = password
        && !pass.is_empty()
    {
        args.push(format!("-password={}", pass));
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
    let mut steam_args = vec!["-applaunch".to_string(), game_id.to_string()];
    // -malloc=system is a Linux-only DayZ optimisation flag; skip on Windows.
    #[cfg(not(target_os = "windows"))]
    steam_args.push("-malloc=system".to_string());
    if let Some(user) = username
        && !user.is_empty()
    {
        steam_args.push(format!("-name={}", user));
    }
    steam_args.extend(args.iter().cloned());
    steam_args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_carry_mods_address_password_and_options() {
        let server = Server {
            game_port: 2402,
            endpoint: dz_api::Endpoint {
                ip: "1.2.3.4".into(),
                port: 27016,
            },
            ..Default::default()
        };
        let args = build_launch_args(
            &server,
            &[1, 2],
            Some("secret"),
            &LaunchOptions::defaults(),
            &["-extra".into()],
        );
        assert_eq!(args[0], "-mod=@1;@2");
        assert!(args.contains(&"-connect=1.2.3.4".to_string()));
        assert!(
            args.contains(&"-port=2402".to_string()),
            "the game port, not the query port"
        );
        assert!(args.contains(&"-password=secret".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("-extra"));

        let none = build_launch_args(&server, &[], Some(""), &LaunchOptions::defaults(), &[]);
        assert!(
            !none
                .iter()
                .any(|a| a.starts_with("-mod=") || a.starts_with("-password="))
        );
    }

    #[test]
    fn steam_wraps_them_with_applaunch_and_the_name() {
        let args = build_steam_applaunch_args(221100, &["-connect=x".into()], Some("Survivor"));
        assert_eq!(&args[..2], ["-applaunch", "221100"]);
        assert!(args.contains(&"-name=Survivor".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("-connect=x"));
    }
}
