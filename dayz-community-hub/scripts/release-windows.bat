@echo off
setlocal enabledelayedexpansion

:: ---------------------------------------------------------------------------
:: release-windows.bat
:: Native Windows build, sign, and publish to Forgejo.
::
:: Usage:
::   cd dayz-community-hub\dayz-community-hub
::   scripts\release-windows.bat
::
:: Requirements:
::   bun, git, curl (Win10+)
::   Signing keys in ..\signing-keys\
::
:: Config (.env in dayz-community-hub\):
::   FORGEJO_TOKEN=your-token-here
:: ---------------------------------------------------------------------------

set "SCRIPT_DIR=%~dp0"
set "UI_DIR=%SCRIPT_DIR%.."
set "REPO_ROOT=%UI_DIR%\.."

:: ---------------------------------------------------------------------------
:: Load .env
:: ---------------------------------------------------------------------------
set "ENV_FILE=%UI_DIR%\.env"
if not exist "%ENV_FILE%" (
    echo ERROR: .env not found at %ENV_FILE%
    echo        Copy .env.example to .env and fill in the values.
    exit /b 1
)
for /f "usebackq tokens=1,* delims==" %%a in ("%ENV_FILE%") do (
    set "%%a=%%b"
)

:: Strip surrounding quotes from values loaded from .env
if defined TAURI_SIGNING_KEY_FILE set "TAURI_SIGNING_KEY_FILE=%TAURI_SIGNING_KEY_FILE:"=%"
if defined TAURI_SIGNING_KEY_PASS set "TAURI_SIGNING_KEY_PASS=%TAURI_SIGNING_KEY_PASS:"=%"

:: Map to local names used throughout this script
set "SIGNING_KEY=%TAURI_SIGNING_KEY_FILE%"
set "SIGNING_KEY_PASS=%TAURI_SIGNING_KEY_PASS%"

:: Validate
if not defined FORGEJO_TOKEN (
    echo ERROR: FORGEJO_TOKEN is not set in %ENV_FILE%
    exit /b 1
)
if not defined SIGNING_KEY (
    echo ERROR: TAURI_SIGNING_KEY_FILE is not set in %ENV_FILE%
    exit /b 1
)
if not exist "%SIGNING_KEY%" (
    echo ERROR: Signing key not found: %SIGNING_KEY%
    exit /b 1
)

:: ---------------------------------------------------------------------------
:: Resolve version + tag
:: ---------------------------------------------------------------------------
set "TAURI_CONF=%UI_DIR%\src-tauri\tauri.conf.json"
for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "(Get-Content '%TAURI_CONF%' | ConvertFrom-Json).version"`) do set "VERSION=%%v"
set "TAG=v%VERSION%"
set "ZIP_NAME=dayz-community-hub-%TAG%-x86_64-windows.zip"

set "FORGEJO_BASE=https://git.thoxy.xyz"
set "REPO_OWNER=thoxy"
set "REPO_NAME=dayz-community-hub"
set "API=%FORGEJO_BASE%/api/v1"

echo ==^> DayZ Community Hub %TAG% — Windows x86_64
echo     Repo     : %FORGEJO_BASE%/%REPO_OWNER%/%REPO_NAME%
echo     Sign key : %SIGNING_KEY%

:: ---------------------------------------------------------------------------
:: 1. Commit, tag, push
:: ---------------------------------------------------------------------------
cd /d "%REPO_ROOT%"
echo.
echo ==^> Committing version bump and pushing...

git add "dayz-community-hub/src-tauri/tauri.conf.json" "dayz-community-hub/src-tauri/Cargo.toml" "dayz-community-hub/package.json" 2>nul

git diff --cached --quiet
if errorlevel 1 (
    git commit -m "chore: bump version to %VERSION%"
    echo     Committed version bump.
) else (
    echo     Nothing to commit (version files already clean^).
)

git push origin master
echo     Pushed master.

git rev-parse "%TAG%" >nul 2>&1
if errorlevel 1 (
    git tag -a "%TAG%" -m "Release %TAG%"
    git push origin "%TAG%"
    echo     Tag %TAG% created and pushed.
) else (
    echo     Tag %TAG% already exists — skipping tag creation.
)

:: ---------------------------------------------------------------------------
:: 2. Build (native Windows)
:: ---------------------------------------------------------------------------
cd /d "%UI_DIR%"
echo.
echo ==^> Installing JS dependencies...
call bun install --frozen-lockfile

echo.
echo ==^> Building for Windows (native, --no-bundle^)...

:: Set signing env vars so Tauri can sign
for /f "usebackq delims=" %%k in (`type "%SIGNING_KEY%"`) do set "TAURI_SIGNING_PRIVATE_KEY=%%k"
:: Actually read the full key content properly (multi-line)
set "TAURI_SIGNING_PRIVATE_KEY="
for /f "usebackq delims=" %%k in (`powershell -NoProfile -Command "[IO.File]::ReadAllText('%SIGNING_KEY%')"`) do (
    if defined TAURI_SIGNING_PRIVATE_KEY (
        set "TAURI_SIGNING_PRIVATE_KEY=!TAURI_SIGNING_PRIVATE_KEY!%%k"
    ) else (
        set "TAURI_SIGNING_PRIVATE_KEY=%%k"
    )
)
set "TAURI_SIGNING_PRIVATE_KEY_PASSWORD=%SIGNING_KEY_PASS%"

call bun tauri build --no-bundle
if errorlevel 1 (
    echo ERROR: Build failed
    exit /b 1
)

set "TARGET_DIR=%UI_DIR%\src-tauri\target\release"
set "EXE=%TARGET_DIR%\dayz-community-hub.exe"

if not exist "%EXE%" (
    echo ERROR: Binary not found: %EXE%
    exit /b 1
)

:: ---------------------------------------------------------------------------
:: 3. Zip (must happen before signing — signature covers the zip)
:: ---------------------------------------------------------------------------
echo.
echo ==^> Zipping (ZIP_STORED / method 0 — required by Tauri updater zip crate^)...
set "ZIP_PATH=%TARGET_DIR%\%ZIP_NAME%"
if exist "%ZIP_PATH%" del "%ZIP_PATH%"
"C:\Program Files\7-Zip\7z.exe" a -tzip -mm=Copy "%ZIP_PATH%" "%EXE%"
if errorlevel 1 (
    echo ERROR: 7-Zip failed. Make sure 7-Zip is installed at "C:\Program Files\7-Zip\7z.exe"
    exit /b 1
)
echo     %ZIP_PATH%

:: ---------------------------------------------------------------------------
:: 4. Sign the zip (signature covers zip bytes, which is what the updater downloads)
:: ---------------------------------------------------------------------------
set "SIG_FILE=%ZIP_PATH%.sig"
echo.
echo ==^> Signing zip...
if defined SIGNING_KEY_PASS (
    call bun tauri signer sign -f "%SIGNING_KEY%" -p "%SIGNING_KEY_PASS%" "%ZIP_PATH%"
) else (
    call bun tauri signer sign -f "%SIGNING_KEY%" "%ZIP_PATH%"
)
if errorlevel 1 (
    echo ERROR: Signing failed
    exit /b 1
)
echo     %SIG_FILE%

:: ---------------------------------------------------------------------------
:: 5. Build latest.json
:: ---------------------------------------------------------------------------
set "LATEST_JSON=%TARGET_DIR%\latest.json"
set "ASSET_URL=%FORGEJO_BASE%/%REPO_OWNER%/%REPO_NAME%/releases/download/%TAG%/%ZIP_NAME%"

echo.
echo ==^> Building latest.json...
powershell -NoProfile -Command ^
    "$sig = (Get-Content '%SIG_FILE%' -Raw).Trim();" ^
    "$pubDate = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ');" ^
    "$json = @{" ^
    "  version = '%VERSION%';" ^
    "  notes = 'Release %TAG%';" ^
    "  pub_date = $pubDate;" ^
    "  platforms = @{" ^
    "    'windows-x86_64' = @{" ^
    "      signature = $sig;" ^
    "      url = '%ASSET_URL%'" ^
    "    }" ^
    "  }" ^
    "} | ConvertTo-Json -Depth 4;" ^
    "Set-Content -Path '%LATEST_JSON%' -Value $json -Encoding UTF8"
echo     %LATEST_JSON%

:: ---------------------------------------------------------------------------
:: 6. Create / reuse versioned Forgejo release
:: ---------------------------------------------------------------------------
echo.
echo ==^> Forgejo release %TAG%...

:: Check if release exists
for /f "usebackq delims=" %%s in (`curl -s -o nul -w "%%{http_code}" -H "Authorization: token %FORGEJO_TOKEN%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/tags/%TAG%"`) do set "HTTP_STATUS=%%s"

if "%HTTP_STATUS%"=="200" (
    for /f "usebackq delims=" %%i in (`curl -s -H "Authorization: token %FORGEJO_TOKEN%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/tags/%TAG%" ^| powershell -NoProfile -Command "$input | ConvertFrom-Json | Select-Object -ExpandProperty id"`) do set "RELEASE_ID=%%i"
    echo     Reusing release %TAG% (id !RELEASE_ID!^)
) else (
    for /f "usebackq delims=" %%i in (`curl -s -X POST -H "Authorization: token %FORGEJO_TOKEN%" -H "Content-Type: application/json" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases" -d "{\"tag_name\":\"%TAG%\",\"name\":\"%TAG%\",\"body\":\"Release %TAG%\",\"draft\":false,\"prerelease\":false}" ^| powershell -NoProfile -Command "$input | ConvertFrom-Json | Select-Object -ExpandProperty id"`) do set "RELEASE_ID=%%i"
    echo     Created release %TAG% (id !RELEASE_ID!^)
)

if not defined RELEASE_ID (
    echo ERROR: Failed to get release ID
    exit /b 1
)

:: Upload assets to versioned release
call :upload_asset %RELEASE_ID% "%ZIP_PATH%"
call :upload_asset %RELEASE_ID% "%LATEST_JSON%"

:: ---------------------------------------------------------------------------
:: 7. Update 'latest' release for updater endpoint
:: ---------------------------------------------------------------------------
echo.
echo ==^> Updating 'latest' release...

for /f "usebackq delims=" %%s in (`curl -s -o nul -w "%%{http_code}" -H "Authorization: token %FORGEJO_TOKEN%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/tags/latest"`) do set "LATEST_STATUS=%%s"

if "%LATEST_STATUS%"=="200" (
    for /f "usebackq delims=" %%i in (`curl -s -H "Authorization: token %FORGEJO_TOKEN%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/tags/latest" ^| powershell -NoProfile -Command "$input | ConvertFrom-Json | Select-Object -ExpandProperty id"`) do set "LATEST_RELEASE_ID=%%i"
    echo     Reusing 'latest' release (id !LATEST_RELEASE_ID!^)
) else (
    for /f "usebackq delims=" %%i in (`curl -s -X POST -H "Authorization: token %FORGEJO_TOKEN%" -H "Content-Type: application/json" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases" -d "{\"tag_name\":\"latest\",\"name\":\"latest\",\"body\":\"Always points to the latest release. Used by the auto-updater.\",\"draft\":false,\"prerelease\":false}" ^| powershell -NoProfile -Command "$input | ConvertFrom-Json | Select-Object -ExpandProperty id"`) do set "LATEST_RELEASE_ID=%%i"
    echo     Created 'latest' release (id !LATEST_RELEASE_ID!^)
)

if not defined LATEST_RELEASE_ID (
    echo ERROR: Failed to get 'latest' release ID
    exit /b 1
)

call :upload_asset %LATEST_RELEASE_ID% "%LATEST_JSON%"

:: ---------------------------------------------------------------------------
:: Done
:: ---------------------------------------------------------------------------
echo.
echo ==^> Done!
echo     Release  : %FORGEJO_BASE%/%REPO_OWNER%/%REPO_NAME%/releases/tag/%TAG%
echo     Updater  : %FORGEJO_BASE%/%REPO_OWNER%/%REPO_NAME%/releases/download/latest/latest.json
exit /b 0

:: ---------------------------------------------------------------------------
:: Helper: upload (or replace) a release asset
:: ---------------------------------------------------------------------------
:upload_asset
set "UA_RELEASE_ID=%~1"
set "UA_FILE=%~2"
set "UA_NAME=%~nx2"

:: Check if asset already exists and delete it
for /f "usebackq delims=" %%i in (`curl -s -H "Authorization: token %FORGEJO_TOKEN%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/%UA_RELEASE_ID%/assets" ^| powershell -NoProfile -Command "$assets = $input | ConvertFrom-Json; ($assets | Where-Object { $_.name -eq '%UA_NAME%' }).id"`) do (
    if not "%%i"=="" (
        curl -s -X DELETE -H "Authorization: token %FORGEJO_TOKEN%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/%UA_RELEASE_ID%/assets/%%i" >nul
        echo     Replaced existing %UA_NAME%
    )
)

:: Upload
curl -s -X POST -H "Authorization: token %FORGEJO_TOKEN%" -F "attachment=@%UA_FILE%;filename=%UA_NAME%" "%API%/repos/%REPO_OWNER%/%REPO_NAME%/releases/%UA_RELEASE_ID%/assets" | powershell -NoProfile -Command "$r = $input | ConvertFrom-Json; Write-Host ('    Uploaded: ' + $r.name)"
exit /b 0
