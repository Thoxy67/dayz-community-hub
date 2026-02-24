#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# release-windows.sh
# Cross-compile the Windows build from Linux (cargo-xwin) and publish a
# tagged release on Forgejo including the Tauri updater latest.json.
#
# Usage:
#   cd dayz-community-hub
#   bash scripts/release-windows.sh
#
# Requirements:
#   cargo install cargo-xwin
#   rustup target add x86_64-pc-windows-msvc
#   bun, jq, minisign, zip
#
# .env (dayz-community-hub/.env):
#   TAURI_SIGNING_KEY_FILE   — path to the minisign .key file
#                              default: ~/.tauri/dayz-community-hub.key
#   TAURI_SIGNING_KEY_PASS   — key password, leave empty if none
#   FORGEJO_TOKEN            — Forgejo API token (write:release scope)
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)" # dayz-community-hub/
REPO_ROOT="$(cd "$UI_DIR/.." && pwd)"  # workspace root

# ---------------------------------------------------------------------------
# Load .env  (look in UI_DIR first, then workspace root)
# ---------------------------------------------------------------------------
SIGNING_KEYS_DIR="/mnt/ssd2/Home/Dev/Rust/a2sdayz/signing-keys"

if [[ -f "$UI_DIR/.env" ]]; then
	ENV_FILE="$UI_DIR/.env"
elif [[ -f "$REPO_ROOT/.env" ]]; then
	ENV_FILE="$REPO_ROOT/.env"
else
	echo "ERROR: .env not found (tried $UI_DIR/.env and $REPO_ROOT/.env)" >&2
	echo "       Copy .env.example to .env and fill in the values." >&2
	exit 1
fi
# shellcheck disable=SC1090
set -a
source "$ENV_FILE"
set +a

# Defaults — key lives in the repo signing-keys directory
TAURI_SIGNING_KEY_FILE="${TAURI_SIGNING_KEY_FILE:-$SIGNING_KEYS_DIR/dayz-community-hub.key}"
TAURI_SIGNING_KEY_PASS="${TAURI_SIGNING_KEY_PASS:-}"

# Validate
for var in FORGEJO_TOKEN; do
	if [[ -z "${!var:-}" ]]; then
		echo "ERROR: $var is not set in $ENV_FILE" >&2
		exit 1
	fi
done
if [[ ! -f "$TAURI_SIGNING_KEY_FILE" ]]; then
	echo "ERROR: signing key not found: $TAURI_SIGNING_KEY_FILE" >&2
	exit 1
fi

# ---------------------------------------------------------------------------
# Resolve version + tag
# ---------------------------------------------------------------------------
TAURI_CONF="$UI_DIR/src-tauri/tauri.conf.json"
VERSION="$(jq -r '.version' "$TAURI_CONF")"
TAG="v$VERSION"
ZIP_NAME="dayz-community-hub-${TAG}-x86_64-windows.zip"

FORGEJO_BASE="https://git.thoxy.xyz"
REPO_OWNER="thoxy"
REPO_NAME="dayz-community-hub"
API="$FORGEJO_BASE/api/v1"

echo "==> DayZ Community Hub $TAG — Windows x86_64"
echo "    Repo     : $FORGEJO_BASE/$REPO_OWNER/$REPO_NAME"
echo "    Env file : $ENV_FILE"
echo "    Sign key : $TAURI_SIGNING_KEY_FILE"

# ---------------------------------------------------------------------------
# 1. Commit, tag, push  (must happen before build so release points to HEAD)
# ---------------------------------------------------------------------------
cd "$REPO_ROOT"
echo ""
echo "==> Committing version bump and pushing..."

# Stage only the version files — anything else is the caller's responsibility
git add \
	dayz-community-hub/src-tauri/tauri.conf.json \
	dayz-community-hub/src-tauri/Cargo.toml \
	dayz-community-hub/package.json 2>/dev/null || true

# Commit only if there are staged changes
if ! git diff --cached --quiet; then
	git commit -m "chore: bump version to $VERSION"
	echo "    Committed version bump."
else
	echo "    Nothing to commit (version files already clean)."
fi

git push origin master
echo "    Pushed master."

if git rev-parse "$TAG" >/dev/null 2>&1; then
	echo "    Tag $TAG already exists — skipping tag creation."
else
	git tag -a "$TAG" -m "Release $TAG"
	git push origin "$TAG"
	echo "    Tag $TAG created and pushed."
fi

# ---------------------------------------------------------------------------
# 2. Build (JS deps + Rust cross-compile, no installer)
# ---------------------------------------------------------------------------
cd "$UI_DIR"
echo ""
echo "==> Installing JS dependencies..."
bun install --frozen-lockfile

echo ""
echo "==> Cross-compiling for Windows (cargo-xwin, --no-bundle)..."
# Pass the raw key content to Tauri's signing mechanism
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_SIGNING_KEY_FILE")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$TAURI_SIGNING_KEY_PASS"

bun tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle

TARGET_DIR="$UI_DIR/src-tauri/target/x86_64-pc-windows-msvc/release"
EXE="$TARGET_DIR/dayz-community-hub.exe"

[[ -f "$EXE" ]] || {
	echo "ERROR: Binary not found: $EXE" >&2
	exit 1
}

# ---------------------------------------------------------------------------
# 3. Sign with minisign
# ---------------------------------------------------------------------------
SIG_FILE="$EXE.sig"
echo ""
echo "==> Signing binary with minisign..."

# The key file is base64-encoded (Tauri/rsign format) — decode it to a
# temporary native minisign key file before passing it to minisign -S.
DECODED_KEY="$(mktemp)"
trap 'rm -f "$DECODED_KEY"' EXIT
base64 -d "$TAURI_SIGNING_KEY_FILE" >"$DECODED_KEY"

if [[ -n "$TAURI_SIGNING_KEY_PASS" ]]; then
	echo "$TAURI_SIGNING_KEY_PASS" | minisign -S -s "$DECODED_KEY" \
		-m "$EXE" -x "$SIG_FILE" -t "dayz-community-hub $TAG"
else
	minisign -S -W -s "$DECODED_KEY" \
		-m "$EXE" -x "$SIG_FILE" -t "dayz-community-hub $TAG"
fi
echo "    $SIG_FILE"

# ---------------------------------------------------------------------------
# 4. Zip
# ---------------------------------------------------------------------------
echo ""
echo "==> Zipping..."
ZIP_PATH="$TARGET_DIR/$ZIP_NAME"
(cd "$TARGET_DIR" && zip -9 "$ZIP_NAME" dayz-community-hub.exe)
echo "    $ZIP_PATH ($(du -sh "$ZIP_PATH" | cut -f1))"

# ---------------------------------------------------------------------------
# 5. Build latest.json
# ---------------------------------------------------------------------------
LATEST_JSON="$TARGET_DIR/latest.json"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
ASSET_URL="$FORGEJO_BASE/$REPO_OWNER/$REPO_NAME/releases/download/$TAG/$ZIP_NAME"

echo ""
echo "==> Building latest.json..."
python3 - <<PYEOF
import json

sig = open("$SIG_FILE").read().rstrip()
data = {
    "version": "$VERSION",
    "notes": "Release $TAG",
    "pub_date": "$PUB_DATE",
    "platforms": {
        "windows-x86_64": {
            "signature": sig,
            "url": "$ASSET_URL",
        }
    },
}
with open("$LATEST_JSON", "w") as f:
    json.dump(data, f, indent=2)
print("    Written:", "$LATEST_JSON")
PYEOF

# ---------------------------------------------------------------------------
# Helper: upload (or replace) a release asset
# ---------------------------------------------------------------------------
upload_asset() {
	local release_id="$1" file="$2"
	local name
	name="$(basename "$file")"

	local existing_id
	existing_id="$(curl -s \
		-H "Authorization: token $FORGEJO_TOKEN" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases/$release_id/assets" |
		jq -r ".[] | select(.name == \"$name\") | .id")"

	if [[ -n "$existing_id" ]]; then
		curl -s -X DELETE \
			-H "Authorization: token $FORGEJO_TOKEN" \
			"$API/repos/$REPO_OWNER/$REPO_NAME/releases/$release_id/assets/$existing_id" \
			>/dev/null
		echo "    Replaced existing $name"
	fi

	curl -s -X POST \
		-H "Authorization: token $FORGEJO_TOKEN" \
		-F "attachment=@$file;filename=$name" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases/$release_id/assets" |
		jq -r '"    Uploaded: \(.name)"'
}

# ---------------------------------------------------------------------------
# 6. Create / reuse versioned Forgejo release
# ---------------------------------------------------------------------------
echo ""
echo "==> Forgejo release $TAG..."

HTTP_STATUS="$(curl -s -o /dev/null -w "%{http_code}" \
	-H "Authorization: token $FORGEJO_TOKEN" \
	"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/$TAG")"

if [[ "$HTTP_STATUS" == "200" ]]; then
	RELEASE_ID="$(curl -s \
		-H "Authorization: token $FORGEJO_TOKEN" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/$TAG" | jq -r '.id')"
	echo "    Reusing release $TAG (id $RELEASE_ID)"
else
	RELEASE_ID="$(curl -s -X POST \
		-H "Authorization: token $FORGEJO_TOKEN" \
		-H "Content-Type: application/json" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases" \
		-d "{\"tag_name\":\"$TAG\",\"name\":\"$TAG\",\"body\":\"Release $TAG\",\"draft\":false,\"prerelease\":false}" |
		jq -r '.id')"
	echo "    Created release $TAG (id $RELEASE_ID)"
fi

[[ -z "$RELEASE_ID" || "$RELEASE_ID" == "null" ]] && {
	echo "ERROR: Failed to get release ID" >&2
	exit 1
}

upload_asset "$RELEASE_ID" "$ZIP_PATH"
upload_asset "$RELEASE_ID" "$LATEST_JSON"

# ---------------------------------------------------------------------------
# 7. Update 'latest' release for updater endpoint
# ---------------------------------------------------------------------------
echo ""
echo "==> Updating 'latest' release..."

LATEST_STATUS="$(curl -s -o /dev/null -w "%{http_code}" \
	-H "Authorization: token $FORGEJO_TOKEN" \
	"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/latest")"

if [[ "$LATEST_STATUS" == "200" ]]; then
	LATEST_RELEASE_ID="$(curl -s \
		-H "Authorization: token $FORGEJO_TOKEN" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/latest" | jq -r '.id')"
	echo "    Reusing 'latest' release (id $LATEST_RELEASE_ID)"
else
	LATEST_RELEASE_ID="$(curl -s -X POST \
		-H "Authorization: token $FORGEJO_TOKEN" \
		-H "Content-Type: application/json" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases" \
		-d '{"tag_name":"latest","name":"latest","body":"Always points to the latest release. Used by the auto-updater.","draft":false,"prerelease":false}' |
		jq -r '.id')"
	echo "    Created 'latest' release (id $LATEST_RELEASE_ID)"
fi

[[ -z "$LATEST_RELEASE_ID" || "$LATEST_RELEASE_ID" == "null" ]] && {
	echo "ERROR: Failed to get 'latest' release ID" >&2
	exit 1
}

upload_asset "$LATEST_RELEASE_ID" "$LATEST_JSON"

# ---------------------------------------------------------------------------
# Done
# ---------------------------------------------------------------------------
echo ""
echo "==> Done!"
echo "    Release  : $FORGEJO_BASE/$REPO_OWNER/$REPO_NAME/releases/tag/$TAG"
echo "    Updater  : $FORGEJO_BASE/$REPO_OWNER/$REPO_NAME/releases/download/latest/latest.json"
