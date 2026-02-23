#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# release-windows.sh
# Cross-compile the Windows build from Linux (cargo-xwin) and publish a
# tagged release on Forgejo including the Tauri updater latest.json.
#
# Requirements:
#   cargo install cargo-xwin
#   rustup target add x86_64-pc-windows-msvc
#   bun, jq, minisign, zip
#
# Secrets (load from .env in the same directory as this script's parent):
#   TAURI_SIGNING_PRIVATE_KEY          — minisign private key (raw text)
#   TAURI_SIGNING_PRIVATE_KEY_PASSWORD — key password (leave empty if none)
#   FORGEJO_TOKEN                      — Forgejo API token (write:release scope)
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)" # dayz-community-hub/
REPO_ROOT="$(cd "$UI_DIR/.." && pwd)"  # workspace root

# ---------------------------------------------------------------------------
# Load secrets from dayz-community-hub/.env
# ---------------------------------------------------------------------------
ENV_FILE="$UI_DIR/.env"
if [[ ! -f "$ENV_FILE" ]]; then
	echo "ERROR: .env not found at $ENV_FILE" >&2
	echo "       Copy dayz-community-hub/.env.example to dayz-community-hub/.env and fill in the values." >&2
	exit 1
fi
# shellcheck disable=SC1090
set -a
source "$ENV_FILE"
set +a

# Validate required vars
for var in TAURI_SIGNING_PRIVATE_KEY FORGEJO_TOKEN; do
	if [[ -z "${!var:-}" ]]; then
		echo "ERROR: $var is not set in $ENV_FILE" >&2
		exit 1
	fi
done

# ---------------------------------------------------------------------------
# Resolve version from tauri.conf.json
# ---------------------------------------------------------------------------
TAURI_CONF="$UI_DIR/src-tauri/tauri.conf.json"
VERSION="$(jq -r '.version' "$TAURI_CONF")"
TAG="v$VERSION"
ZIP_NAME="dayz-community-hub-${TAG}-x86_64-windows.zip"

FORGEJO_BASE="https://git.thoxy.xyz"
REPO_OWNER="thoxy"
REPO_NAME="dayz-community-hub"
API="$FORGEJO_BASE/api/v1"

echo "==> DayZ Community Hub $TAG — Windows x86_64 release"
echo "    Repo : $FORGEJO_BASE/$REPO_OWNER/$REPO_NAME"
echo "    .env : $ENV_FILE"

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
cd "$UI_DIR"

echo ""
echo "==> Installing JS dependencies..."
bun install --frozen-lockfile

echo ""
echo "==> Cross-compiling for Windows (cargo-xwin, no-bundle)..."
export TAURI_SIGNING_PRIVATE_KEY
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

bun tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle

# ---------------------------------------------------------------------------
# Sign the binary with minisign
# ---------------------------------------------------------------------------
TARGET_DIR="$UI_DIR/src-tauri/target/x86_64-pc-windows-msvc/release"
EXE="$TARGET_DIR/dayz-community-hub.exe"
SIG_FILE="$EXE.sig"

if [[ ! -f "$EXE" ]]; then
	echo "ERROR: Binary not found: $EXE" >&2
	exit 1
fi

echo ""
echo "==> Signing binary with minisign..."

# Write private key to a temp file so minisign can read it
KEY_TMP="$(mktemp)"
trap 'rm -f "$KEY_TMP"' EXIT
printf '%s' "$TAURI_SIGNING_PRIVATE_KEY" >"$KEY_TMP"

MINISIGN_PASS="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
if [[ -n "$MINISIGN_PASS" ]]; then
	echo "$MINISIGN_PASS" | minisign -S -s "$KEY_TMP" -m "$EXE" -x "$SIG_FILE" \
		-t "dayz-community-hub $TAG" -p /dev/stdin
else
	minisign -S -s "$KEY_TMP" -m "$EXE" -x "$SIG_FILE" \
		-t "dayz-community-hub $TAG"
fi
echo "    Signature: $SIG_FILE"

# ---------------------------------------------------------------------------
# Zip the binary
# ---------------------------------------------------------------------------
echo ""
echo "==> Zipping binary..."
ZIP_PATH="$TARGET_DIR/$ZIP_NAME"
(cd "$TARGET_DIR" && zip -9 "$ZIP_NAME" dayz-community-hub.exe)
echo "    Archive: $ZIP_PATH ($(du -sh "$ZIP_PATH" | cut -f1))"

# ---------------------------------------------------------------------------
# Build latest.json (signature embedded as proper JSON string)
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
# Git tag (skip if already exists)
# ---------------------------------------------------------------------------
echo ""
echo "==> Tagging $TAG..."
cd "$REPO_ROOT"
if git rev-parse "$TAG" >/dev/null 2>&1; then
	echo "    Tag $TAG already exists — skipping."
else
	git tag -a "$TAG" -m "Release $TAG"
	git push origin "$TAG"
	echo "    Tag $TAG pushed."
fi

# ---------------------------------------------------------------------------
# Create / fetch the versioned Forgejo release
# ---------------------------------------------------------------------------
echo ""
echo "==> Creating Forgejo release $TAG..."

HTTP_STATUS="$(curl -s -o /dev/null -w "%{http_code}" \
	-H "Authorization: token $FORGEJO_TOKEN" \
	"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/$TAG")"

if [[ "$HTTP_STATUS" == "200" ]]; then
	echo "    Release $TAG already exists — reusing it."
	RELEASE_ID="$(curl -s \
		-H "Authorization: token $FORGEJO_TOKEN" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/$TAG" | jq -r '.id')"
else
	RELEASE_ID="$(curl -s -X POST \
		-H "Authorization: token $FORGEJO_TOKEN" \
		-H "Content-Type: application/json" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases" \
		-d "{
      \"tag_name\": \"$TAG\",
      \"name\": \"$TAG\",
      \"body\": \"Release $TAG\",
      \"draft\": false,
      \"prerelease\": false
    }" | jq -r '.id')"
fi

[[ -z "$RELEASE_ID" || "$RELEASE_ID" == "null" ]] && {
	echo "ERROR: Failed to get release ID" >&2
	exit 1
}
echo "    Release ID: $RELEASE_ID"

# ---------------------------------------------------------------------------
# Helper: upload (or replace) a release asset
# ---------------------------------------------------------------------------
upload_asset() {
	local release_id="$1"
	local file="$2"
	local name
	name="$(basename "$file")"

	# Delete existing asset with the same name if present
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
		jq -r '"    Uploaded: \(.name) -> \(.browser_download_url)"'
}

# ---------------------------------------------------------------------------
# Upload assets to versioned release
# ---------------------------------------------------------------------------
echo ""
echo "==> Uploading assets to $TAG release..."
upload_asset "$RELEASE_ID" "$ZIP_PATH"
upload_asset "$RELEASE_ID" "$LATEST_JSON"

# ---------------------------------------------------------------------------
# Update the 'latest' release (used by the updater endpoint)
# ---------------------------------------------------------------------------
echo ""
echo "==> Updating 'latest' release for updater endpoint..."

LATEST_STATUS="$(curl -s -o /dev/null -w "%{http_code}" \
	-H "Authorization: token $FORGEJO_TOKEN" \
	"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/latest")"

if [[ "$LATEST_STATUS" == "200" ]]; then
	LATEST_RELEASE_ID="$(curl -s \
		-H "Authorization: token $FORGEJO_TOKEN" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases/tags/latest" | jq -r '.id')"
	echo "    Reusing existing 'latest' release (id $LATEST_RELEASE_ID)"
else
	LATEST_RELEASE_ID="$(curl -s -X POST \
		-H "Authorization: token $FORGEJO_TOKEN" \
		-H "Content-Type: application/json" \
		"$API/repos/$REPO_OWNER/$REPO_NAME/releases" \
		-d '{
      "tag_name": "latest",
      "name": "latest",
      "body": "Always points to the latest release. Used by the auto-updater.",
      "draft": false,
      "prerelease": false
    }' | jq -r '.id')"
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
echo "==> Release complete!"
echo "    Versioned : $FORGEJO_BASE/$REPO_OWNER/$REPO_NAME/releases/tag/$TAG"
echo "    Updater   : $FORGEJO_BASE/$REPO_OWNER/$REPO_NAME/releases/download/latest/latest.json"
