//! The updater's signatures: `cargo xtask sign` and `cargo xtask keygen`.
//!
//! What `tauri signer sign` / `tauri signer generate` did, through the same crate the
//! Tauri CLI uses (`minisign`), so nothing changes for the files or the key:
//!
//! - the **key file** is base64 of a minisign secret key file (two lines, the key
//!   scrypt-encrypted with the password; the password may be empty);
//! - a **`.sig`** is base64 of a minisign signature file: prehashed (`ED`), untrusted
//!   comment `signature from tauri secret key`, trusted comment
//!   `timestamp:<unix seconds>\tfile:<file name>`, no trailing newline;
//! - a **public key** is base64 of the minisign public key file, as `tauri.conf.json`
//!   held it and `dz-hub`'s updater (`release::PUBKEY`) holds it now.
//!
//! The password comes from `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty when unset),
//! the variable `dzch release` has always set; nothing is ever prompted for, printed
//! or written but the `.sig` (and, for `keygen`, the new key pair).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use minisign::{KeyPair, SecretKey, SecretKeyBox};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// The untrusted comment Tauri's signer writes.
const UNTRUSTED: &str = "signature from tauri secret key";
/// The environment variable holding the key's password.
pub const PASSWORD_ENV: &str = "TAURI_SIGNING_PRIVATE_KEY_PASSWORD";

/// `cargo xtask sign --key <file> <file>...` and `cargo xtask keygen --write <file>`.
pub fn run(command: &str, args: &[&str]) -> Result<(), String> {
    match command {
        "sign" => {
            let (key, files) = split_flag(args, "--key")?;
            if files.is_empty() {
                return Err("sign: name at least one file to sign".into());
            }
            let password = std::env::var(PASSWORD_ENV).unwrap_or_default();
            let sk = load_secret_key(&read_key_file(Path::new(&key))?, &password)?;
            for file in files {
                let sig = sign_file(&sk, Path::new(file))?;
                println!("signed {file} -> {}", sig.display());
            }
            Ok(())
        }
        "keygen" => {
            let (out, rest) = split_flag(args, "--write")?;
            if !rest.is_empty() {
                return Err(format!("keygen: unexpected arguments {rest:?}"));
            }
            let pubkey = keygen(Path::new(&out))?;
            println!("public key (dz-hub's updater release::PUBKEY):\n{pubkey}");
            Ok(())
        }
        other => Err(format!("unknown signing command `{other}`")),
    }
}

/// `<flag> <value>` taken out of `args`; the value and what is left.
fn split_flag<'a>(args: &[&'a str], flag: &str) -> Result<(String, Vec<&'a str>), String> {
    let mut value = None;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if *a == flag {
            value = it.next().map(|v| (*v).to_owned());
        } else if let Some(v) = a.strip_prefix(&format!("{flag}=")) {
            value = Some(v.to_owned());
        } else {
            rest.push(*a);
        }
    }
    let value = value.ok_or_else(|| format!("{flag} <file> is required"))?;
    Ok((value, rest))
}

fn read_key_file(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("signing key {}: {e}", path.display()))
}

/// The secret key from the key file's text (base64 of the minisign secret key
/// file, Tauri's format) and its password. An unencrypted minisign key is accepted
/// too (then the password is not used).
pub fn load_secret_key(key_file_text: &str, password: &str) -> Result<SecretKey, String> {
    let raw = B64
        .decode(key_file_text.trim())
        .map_err(|e| format!("signing key: not base64 ({e})"))?;
    let text = String::from_utf8(raw).map_err(|_| "signing key: not a minisign key file")?;
    match SecretKeyBox::from_string(&text)
        .and_then(|b| b.into_secret_key(Some(password.to_owned())))
    {
        Ok(sk) => Ok(sk),
        Err(e) if e.to_string().contains("not encrypted") => SecretKeyBox::from_string(&text)
            .and_then(SecretKeyBox::into_unencrypted_secret_key)
            .map_err(|e| format!("signing key: {e}")),
        Err(e) => Err(format!("signing key: {e}")),
    }
}

/// The `.sig` text (base64) for `data`, signed as `file_name`, at `timestamp`.
pub fn signature(
    sk: &SecretKey,
    data: &[u8],
    file_name: &str,
    timestamp: u64,
) -> Result<String, String> {
    let trusted = format!("timestamp:{timestamp}\tfile:{file_name}");
    let sig = minisign::sign(None, sk, data, Some(&trusted), Some(UNTRUSTED))
        .map_err(|e| format!("signing {file_name}: {e}"))?;
    Ok(B64.encode(sig.to_string()))
}

/// Sign `path` into `<path>.sig` (`app.AppImage` → `app.AppImage.sig`), replacing an
/// older one; the `.sig`'s path.
pub fn sign_file(sk: &SecretKey, path: &Path) -> Result<PathBuf, String> {
    let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{}: not a file", path.display()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let text = signature(sk, &data, &name, timestamp)?;
    let mut sig = path.as_os_str().to_owned();
    sig.push(".sig");
    let sig = PathBuf::from(sig);
    std::fs::write(&sig, text).map_err(|e| format!("{}: {e}", sig.display()))?;
    Ok(sig)
}

/// A new key pair in Tauri's format, encrypted with `password`: (key file text,
/// public key), both base64.
pub fn generate(password: &str) -> Result<(String, String), String> {
    let KeyPair { pk, sk } = KeyPair::generate_encrypted_keypair(Some(password.to_owned()))
        .map_err(|e| format!("keygen: {e}"))?;
    let sk_box = sk.to_box(None).map_err(|e| format!("keygen: {e}"))?;
    let pk_box = pk.to_box().map_err(|e| format!("keygen: {e}"))?;
    Ok((
        B64.encode(sk_box.into_string()),
        B64.encode(pk_box.into_string()),
    ))
}

/// What `tauri signer generate --ci -p "" -w <out>` did: a new key without a password
/// at `out` (owner-only permissions), its public key at `<out>.pub`. Refuses to
/// replace an existing file (`dzch keys` moves the old key aside first). The public key.
fn keygen(out: &Path) -> Result<String, String> {
    let mut pub_path = out.as_os_str().to_owned();
    pub_path.push(".pub");
    let pub_path = PathBuf::from(pub_path);
    for p in [out, pub_path.as_path()] {
        if p.exists() {
            return Err(format!("{} exists: move it aside first", p.display()));
        }
    }
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let (sk, pk) = generate("")?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(out)
        .map_err(|e| format!("{}: {e}", out.display()))?;
    f.write_all(sk.as_bytes())
        .map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(&pub_path, &pk).map_err(|e| format!("{}: {e}", pub_path.display()))?;
    Ok(pk)
}

#[cfg(test)]
mod tests {
    //! A throwaway key made here, never the owner's: signed with this module, checked
    //! with the updater's own verification and, when installed, the minisign CLI.
    use super::*;

    #[test]
    fn a_signature_passes_the_updaters_check() {
        let (key_file, pubkey) = generate("throwaway password").unwrap();
        let sk = load_secret_key(&key_file, "throwaway password").unwrap();
        let data = b"an AppImage, or the Windows zip".repeat(1000);
        let sig = signature(&sk, &data, "dayz-community-hub.AppImage", 1_760_318_032).unwrap();
        dz_hub::features::updater::verify_update_signature(&data, &sig, &pubkey).unwrap();

        let mut tampered = data.clone();
        tampered[10] ^= 1;
        assert!(
            dz_hub::features::updater::verify_update_signature(&tampered, &sig, &pubkey).is_err()
        );
        let (_, other) = generate("").unwrap();
        assert!(dz_hub::features::updater::verify_update_signature(&data, &sig, &other).is_err());
    }

    /// A throwaway key pair and a signature made by minisign 0.7.3 (the crate the Tauri
    /// CLI used when the owner's key was made), in Tauri's base64 format, password
    /// `fixture password`: an existing Tauri key must load and sign here, and what it
    /// signs must pass the updater's check.
    const TAURI_SK: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5ClJXUlRZMEl5MzNBajRabGhobC81UnlMRWVneXdJeHBoaWY3ZGN6ZDBLd1MwZWhLSVBrZ0FBQkFBQUFBQUFBQUFBQUlBQUFBQUx3TGFMa2xhdW5EbjNTK2I0Q0tpK0RtbW1xOW8xMHcyRFFKS2UvYjF2TGVqR2xNMlpJazFXbGt0RkFXTzVROHc5dU1NdkdJd1g4enlzZ1cydmFKMHRmTGROMkQwOGZLU1lQc2lZODRaL1dGT0RSdmNEZmh0ekZqVUd0eVArMnpmL0FjYUJyZ1hONWs9Cg==";
    const TAURI_PK: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDNDNUYyM0VCMUQwRThGM0EKUldRNmp3NGQ2eU5mUEVSMnczSUNZUEZBN0Q0d0l3NkFXY25JUFE4cnlRU1pwbVBzQ2VROWFJWjYK";
    const TAURI_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRNmp3NGQ2eU5mUEhUYXgyNHpOS1NFaTUwTXJNeHMva2xIeUlxc1pQYVM3N2tISEZEUTJwOUxBT3J0Uks4NUFWa2xqeDhSUnJNQll3dVVJUVF1VjZKbHhUa2F0QVE4c1FzPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxCWZpbGU6YS56aXAKMDBUdDdSN2tKbjIvYUZGODJGeTd2L2JkT0h1RnR3aDFXMHI2MWZhYllwL3FqMkRQcVlLWUNmdGlLUVNNRTZnZmM0ZzY4NFBKUHNWbXhlMkVmeFBpQ0E9PQo=";

    #[test]
    fn a_key_made_by_the_tauri_era_minisign_signs_here() {
        use dz_hub::features::updater::verify_update_signature as verify;
        verify(b"signed by minisign 0.7", TAURI_SIG, TAURI_PK).unwrap();
        let sk = load_secret_key(TAURI_SK, "fixture password").unwrap();
        let data = b"the next release".repeat(100);
        let sig = signature(&sk, &data, "a.AppImage", 1_760_318_032).unwrap();
        verify(&data, &sig, TAURI_PK).unwrap();
        assert!(load_secret_key(TAURI_SK, "").is_err());
    }

    #[test]
    fn the_sig_file_is_what_tauri_wrote() {
        let (key_file, _) = generate("").unwrap();
        let sk = load_secret_key(&key_file, "").unwrap();
        let sig = signature(&sk, b"x", "a.zip", 42).unwrap();
        let text = String::from_utf8(B64.decode(&sig).unwrap()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "untrusted comment: signature from tauri secret key"
        );
        assert!(
            B64.decode(lines[1]).unwrap().starts_with(b"ED"),
            "prehashed `ED` signature"
        );
        assert_eq!(lines[2], "trusted comment: timestamp:42\tfile:a.zip");
        assert_eq!(lines.len(), 4);
        assert!(!sig.ends_with('\n'));
    }

    #[test]
    fn a_wrong_password_or_a_broken_key_is_an_error() {
        let (key_file, _) = generate("right").unwrap();
        let err = load_secret_key(&key_file, "wrong").err().unwrap();
        assert!(err.contains("password"), "{err}");
        assert!(load_secret_key("not base64!", "").is_err());
        assert!(load_secret_key(&B64.encode("two\nlines"), "").is_err());
    }

    #[test]
    fn sign_file_writes_the_sig_beside_the_file_and_keygen_refuses_to_overwrite() {
        let dir = std::env::temp_dir().join(format!("dzch-xtask-sign-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let key = dir.join("updater.key");
        let pubkey = keygen(&key).unwrap();
        assert!(keygen(&key).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&key).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("updater.key.pub")).unwrap(),
            pubkey
        );
        let sk = load_secret_key(&std::fs::read_to_string(&key).unwrap(), "").unwrap();
        let file = dir.join("app.AppImage");
        std::fs::write(&file, b"image bytes").unwrap();
        let sig_path = sign_file(&sk, &file).unwrap();
        assert_eq!(sig_path, dir.join("app.AppImage.sig"));
        let sig = std::fs::read_to_string(&sig_path).unwrap();
        dz_hub::features::updater::verify_update_signature(b"image bytes", &sig, &pubkey).unwrap();

        // The minisign CLI agrees, when it is installed.
        if crate::on_path("minisign") {
            let decode = |s: &str| String::from_utf8(B64.decode(s).unwrap()).unwrap();
            std::fs::write(dir.join("key.pub"), decode(&pubkey)).unwrap();
            std::fs::write(dir.join("app.minisig"), decode(&sig)).unwrap();
            let out = std::process::Command::new("minisign")
                .args(["-V", "-p"])
                .arg(dir.join("key.pub"))
                .arg("-m")
                .arg(&file)
                .arg("-x")
                .arg(dir.join("app.minisig"))
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "minisign -V: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
