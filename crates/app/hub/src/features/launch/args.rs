//! A favourite's own character name and launch arguments, merged with the
//! global launch options when that server is joined.
//!
//! The rules:
//!
//! 1. **Layers.** The global launch options (the options page) come first,
//!    then the favourite's own arguments, then the ones the join itself carries
//!    (Direct Connect's extra arguments). A later layer is more specific.
//! 2. **One value per flag.** A flag is named by what comes before its `=`,
//!    case aside (`-maxMem=4096` and `-maxmem=8192` are both `-maxmem`). When
//!    a later layer (or a later word of the same layer) names a flag again, it
//!    replaces the earlier one, at its own place. `-mod=` and `-serverMod=` are
//!    the exception: they list mods one per argument, so only the very same
//!    argument twice is dropped.
//! 3. **The join is the launcher's.** `-connect=`, `-port=` and `-password=`
//!    in a favourite's arguments are dropped (the launcher writes them from the
//!    server joined), and so are `-mod=` / `-serverMod=` (the server's mods come
//!    from the server; extra mods are Direct Connect's).
//! 4. **The character name.** The favourite's character name wins; else the
//!    last `-name=` in its arguments or the join's; else the profile's player
//!    name. Every `-name=` is taken out of the arguments: Steam's command line
//!    carries exactly one.
//! 5. **The allocator.** On Linux the launcher adds `-malloc=system` unless
//!    the merged arguments name an allocator of their own
//!    (`dz_game::launch::build_steam_applaunch_args`).
//!
//! Trust: a favourite's arguments go on the command line as written, like
//! the global options' values, and an imported profile brings its own; only
//! the flags of rule 3 are held back.

/// What the launch hands Steam for one join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The character name (`-name=`), if any.
    pub name: Option<String>,
    /// The launch options and extra arguments, merged, in order.
    pub args: Vec<String>,
}

/// Flags that name a mod each: kept side by side, only exact repeats dropped.
const REPEATABLE: [&str; 2] = ["-mod", "-servermod"];
/// Flags the launcher writes from the server joined.
const RESERVED: [&str; 5] = ["-connect", "-port", "-password", "-mod", "-servermod"];

/// What a flag is known by: the text before its `=`, lowercased.
pub fn flag_key(arg: &str) -> String {
    arg.split_once('=')
        .map_or(arg, |(k, _)| k)
        .trim()
        .to_lowercase()
}

/// A line of arguments as typed, split on spaces; double or single quotes keep
/// spaces in one argument (`-profiles="C:\My Profiles"`), and are taken off.
pub fn split_args(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                any = true;
            }
            None if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            None => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out.retain(|a| !a.is_empty());
    out
}

/// Merge the layers by the rules above. `options` are the global launch
/// options' arguments, `favorite` the favourite's own line (empty for a server
/// that is not one), `join` the join's own arguments; `character` the
/// favourite's character name and `player` the profile's.
pub fn merge(
    options: &[String],
    favorite: &str,
    join: &[String],
    character: Option<&str>,
    player: Option<&str>,
) -> Merged {
    let favorite: Vec<String> = split_args(favorite)
        .into_iter()
        .filter(|a| !RESERVED.contains(&flag_key(a).as_str()))
        .collect();
    let mut out: Vec<String> = Vec::new();
    let mut named: Option<String> = None;
    for a in options.iter().chain(&favorite).chain(join) {
        let a = a.trim();
        if a.is_empty() {
            continue;
        }
        let key = flag_key(a);
        if key == "-name" {
            let v = a.split_once('=').map_or("", |(_, v)| v).trim();
            if !v.is_empty() {
                named = Some(v.to_string());
            }
            continue;
        }
        if REPEATABLE.contains(&key.as_str()) {
            out.retain(|b| b != a);
        } else {
            out.retain(|b| flag_key(b) != key);
        }
        out.push(a.to_string());
    }
    let clean = |s: Option<&str>| s.map(str::trim).filter(|s| !s.is_empty()).map(String::from);
    Merged {
        name: clean(character).or(named).or_else(|| clean(player)),
        args: out,
    }
}

/// The favourite a join goes to: saved under the address joined (the name
/// typed, or the IP) with its game or query port.
pub fn favorite_for<'a>(
    favorites: &'a [dz_profile::Favorite],
    hosts: &[&str],
    ports: &[u16],
) -> Option<&'a dz_profile::Favorite> {
    favorites.iter().find(|f| {
        hosts.iter().any(|h| f.ip.trim().eq_ignore_ascii_case(h)) && ports.contains(&f.port)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|a| (*a).to_string()).collect()
    }

    #[test]
    fn splitting_keeps_quoted_spaces() {
        assert_eq!(
            split_args(r#"  -nosplash -profiles="C:\My Profiles"  'x y' "" -a=1 "#),
            v(&["-nosplash", r"-profiles=C:\My Profiles", "x y", "-a=1"])
        );
        assert!(split_args("   ").is_empty());
        // An unclosed quote runs to the end rather than failing.
        assert_eq!(split_args(r#"-a "b c"#), v(&["-a", "b c"]));
    }

    #[test]
    fn a_favorite_flag_replaces_the_global_one_in_its_place() {
        let m = merge(
            &v(&["-nosplash", "-maxMem=8192", "-world=empty"]),
            "-maxmem=4096 -window",
            &[],
            None,
            None,
        );
        assert_eq!(
            m.args,
            v(&["-nosplash", "-world=empty", "-maxmem=4096", "-window"])
        );
    }

    #[test]
    fn exact_and_same_flag_repeats_go_and_the_last_wins() {
        let m = merge(
            &v(&["-nosplash", "-skipIntro"]),
            "-nosplash -cpuCount=2 -cpuCount=4",
            &v(&["-skipintro"]),
            None,
            None,
        );
        assert_eq!(m.args, v(&["-nosplash", "-cpuCount=4", "-skipintro"]));
    }

    #[test]
    fn the_join_is_the_launchers_and_mods_stay_side_by_side() {
        let m = merge(
            &v(&["-nosplash"]),
            "-connect=6.6.6.6 -port=1 -password=x -mod=@9 -doLogs",
            &v(&["-mod=@1", "-mod=@2", "-mod=@1"]),
            None,
            None,
        );
        assert_eq!(m.args, v(&["-nosplash", "-doLogs", "-mod=@2", "-mod=@1"]));
    }

    #[test]
    fn the_character_name_wins_over_name_flags_and_the_player() {
        let opts = v(&["-nosplash"]);
        let m = merge(
            &opts,
            "-name=FromArgs",
            &[],
            Some(" Alt "),
            Some("Survivor"),
        );
        assert_eq!(m.name.as_deref(), Some("Alt"));
        assert_eq!(m.args, v(&["-nosplash"]));
        let m = merge(&opts, "-name=FromArgs", &[], Some("  "), Some("Survivor"));
        assert_eq!(m.name.as_deref(), Some("FromArgs"));
        let m = merge(&opts, "-name=", &[], None, Some("Survivor"));
        assert_eq!(m.name.as_deref(), Some("Survivor"));
        let m = merge(&opts, "", &[], None, None);
        assert_eq!(m.name, None);
    }

    #[test]
    fn a_favorite_is_found_by_name_or_ip_and_either_port() {
        let fav = |ip: &str, port| dz_profile::Favorite {
            ip: ip.into(),
            port,
            ..dz_profile::Favorite::default()
        };
        let favs = [fav("1.2.3.4", 27016), fav("play.example.org", 2302)];
        assert!(favorite_for(&favs, &["1.2.3.4"], &[2302, 27016]).is_some());
        assert!(favorite_for(&favs, &["Play.Example.org", "5.5.5.5"], &[2302]).is_some());
        assert!(favorite_for(&favs, &["1.2.3.4"], &[2302]).is_none());
    }
}
