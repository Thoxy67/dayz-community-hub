$ErrorActionPreference = 'Stop'

# ── Paths ─────────────────────────────────────────────────────────────────────
$scriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
$uiDir      = (Resolve-Path "$scriptDir\..").Path
$repoRoot   = (Resolve-Path "$uiDir\..").Path

$forgejo    = 'https://git.thoxy.xyz'
$owner      = 'thoxy'
$repo       = 'dayz-community-hub'
$api        = "$forgejo/api/v1"

# ── Load .env ─────────────────────────────────────────────────────────────────
$envFile = "$uiDir\.env"
if (-not (Test-Path $envFile)) { throw ".env not found at $envFile — copy .env.example and fill in the values." }
$env = @{}
Get-Content $envFile | Where-Object { $_ -match '^\s*[^#]\S*=' } | ForEach-Object {
    $parts = $_ -split '=', 2
    $env[$parts[0].Trim()] = $parts[1].Trim().Trim('"').Trim("'")
}

$token   = $env['FORGEJO_TOKEN']
$keyFile = $env['TAURI_SIGNING_KEY_FILE']
$keyPass = $env['TAURI_SIGNING_KEY_PASS']   # may be empty

if (-not $token)   { throw 'FORGEJO_TOKEN is not set in .env' }
if (-not $keyFile) { throw 'TAURI_SIGNING_KEY_FILE is not set in .env' }
if (-not (Test-Path $keyFile)) { throw "Signing key not found: $keyFile" }
$hdrs = @{ Authorization = "token $token" }

# ── Resolve version ───────────────────────────────────────────────────────────
$targetDir = "$uiDir\src-tauri\target\release"
$version = (Get-Content "$uiDir\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$tag     = "v$version"
$zipName = "dayz-community-hub-$tag-x86_64-windows.zip"
$exe     = "$targetDir\dayz-community-hub.exe"
$zipPath = "$targetDir\$zipName"
$sigPath = "$zipPath.sig"
$jsonPath = "$targetDir\latest.json"

Write-Host "==> DayZ Community Hub $tag"
if (-not (Test-Path $exe)) { throw "exe not found: $exe" }
Write-Host "    exe: $((Get-Item $exe).Length) bytes"

# ── 1. Git: commit any staged version files, tag, push ───────────────────────
Write-Host ""
Write-Host "==> Git commit / tag / push..."
Set-Location $repoRoot

git add "dayz-community-hub/src-tauri/tauri.conf.json" `
        "dayz-community-hub/src-tauri/Cargo.toml" `
        "dayz-community-hub/package.json" `
        "Cargo.toml" `
        "Cargo.lock" 2>$null

$staged = git diff --cached --quiet 2>$null; $hasStagedChanges = ($LASTEXITCODE -ne 0)
if ($hasStagedChanges) {
    git commit -m "chore: bump version to $version"
    Write-Host "    Committed version bump."
} else {
    Write-Host "    Nothing to commit."
}

git push origin master
Write-Host "    Pushed master."

$ErrorActionPreference = 'Continue'
git rev-parse $tag 2>$null | Out-Null; $tagMissing = ($LASTEXITCODE -ne 0)
$ErrorActionPreference = 'Stop'
if ($tagMissing) {
    git tag -a $tag -m "Release $tag"
    git push origin $tag
    Write-Host "    Tag $tag created and pushed."
} else {
    Write-Host "    Tag $tag already exists - skipping."
}

# 2. Zip then sign (signature covers zip bytes, which is what the updater downloads)
Write-Host ""
Write-Host "==> Zipping with ZIP_STORED (method 0)..."
if (Test-Path $zipPath) { Remove-Item $zipPath }
& 'C:\Program Files\7-Zip\7z.exe' a -tzip -mm=Copy $zipPath $exe | Out-Null
$zipMB = [math]::Round((Get-Item $zipPath).Length / 1048576, 2)
Write-Host "    $zipPath ($zipMB MB)"

Write-Host ""
Write-Host "==> Signing zip..."
if (Test-Path $sigPath) { Remove-Item $sigPath }
Set-Location $uiDir
$ErrorActionPreference = 'Continue'
if ($keyPass) {
    & bun tauri signer sign -f $keyFile -p $keyPass $zipPath 2>&1 | Write-Host
} else {
    & bun tauri signer sign -f $keyFile $zipPath 2>&1 | Write-Host
}
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $sigPath)) { throw 'Signing failed - .sig not created' }
$sig = (Get-Content $sigPath -Raw).Trim()
Write-Host "    Signature OK"

# ── 3. Build latest.json ──────────────────────────────────────────────────────
Write-Host ""
Write-Host "==> Building latest.json..."
$assetUrl = "$forgejo/$owner/$repo/releases/download/$tag/$zipName"
$pubDate  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
$obj = [ordered]@{
    version  = $version
    notes    = "Release $tag"
    pub_date = $pubDate
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = $sig
            url       = $assetUrl
        }
    }
}
[System.IO.File]::WriteAllText($jsonPath, ($obj | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))
Write-Host "    $jsonPath"

# ── Helper ────────────────────────────────────────────────────────────────────
function Replace-Asset($releaseId, $file) {
    $name   = Split-Path $file -Leaf
    $assets = Invoke-RestMethod -Uri "$api/repos/$owner/$repo/releases/$releaseId/assets" -Headers $hdrs
    $old    = $assets | Where-Object { $_.name -eq $name }
    if ($old) {
        Invoke-RestMethod -Method DELETE -Uri "$api/repos/$owner/$repo/releases/$releaseId/assets/$($old.id)" -Headers $hdrs | Out-Null
        Write-Host "    Deleted old $name"
    }
    $r = curl.exe -s -X POST -H "Authorization: token $token" -F "attachment=@$file;filename=$name" "$api/repos/$owner/$repo/releases/$releaseId/assets"
    Write-Host "    Uploaded: $(($r | ConvertFrom-Json).name)"
}

# ── 4. Create / reuse versioned Forgejo release ───────────────────────────────
Write-Host ""
Write-Host "==> Forgejo release $tag..."
$status = curl.exe -s -o NUL -w "%{http_code}" -H "Authorization: token $token" "$api/repos/$owner/$repo/releases/tags/$tag"
if ($status -eq '200') {
    $releaseId = (Invoke-RestMethod -Uri "$api/repos/$owner/$repo/releases/tags/$tag" -Headers $hdrs).id
    Write-Host "    Reusing release $tag (id $releaseId)"
} else {
    $body = @{ tag_name = $tag; name = $tag; body = "Release $tag"; draft = $false; prerelease = $false } | ConvertTo-Json
    $releaseId = (Invoke-RestMethod -Method POST -Uri "$api/repos/$owner/$repo/releases" -Headers $hdrs -ContentType 'application/json' -Body $body).id
    Write-Host "    Created release $tag (id $releaseId)"
}

Replace-Asset $releaseId $zipPath
Replace-Asset $releaseId $jsonPath

# ── 5. Update 'latest' release ────────────────────────────────────────────────
Write-Host ""
Write-Host "==> Updating 'latest' release..."
$lstatus = curl.exe -s -o NUL -w "%{http_code}" -H "Authorization: token $token" "$api/repos/$owner/$repo/releases/tags/latest"
if ($lstatus -eq '200') {
    $latestId = (Invoke-RestMethod -Uri "$api/repos/$owner/$repo/releases/tags/latest" -Headers $hdrs).id
    Write-Host "    Reusing latest release (id $latestId)"
} else {
    # Ensure 'latest' git tag exists (Forgejo requires the tag before creating a release)
    $ErrorActionPreference = 'Continue'
    git tag -f latest 2>&1 | Out-Null
    git push -f origin latest 2>&1 | Out-Null
    $ErrorActionPreference = 'Stop'
    Write-Host "    Ensured git tag 'latest' exists."
    $lbody = '{"tag_name":"latest","name":"latest","body":"Always points to the latest release. Used by the auto-updater.","draft":false,"prerelease":false}'
    $latestId = (Invoke-RestMethod -Method POST -Uri "$api/repos/$owner/$repo/releases" -Headers $hdrs -ContentType 'application/json' -Body $lbody).id
    Write-Host "    Created latest release (id $latestId)"
}

Replace-Asset $latestId $jsonPath

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "==> Done!"
Write-Host "    Release : $forgejo/$owner/$repo/releases/tag/$tag"
Write-Host "    Updater : $forgejo/$owner/$repo/releases/download/latest/latest.json"
