#Requires -Version 5.1
<#
install.ps1 - one-time installer for the ai-meta `meta` CLI (native Windows).

  irm https://raw.githubusercontent.com/tizz98/ai-meta/main/install.ps1 | iex

Downloads the `meta` binary from GitHub releases, verifies its checksum, and
installs it. After installing, run `meta init` in a repo.

Tunables (environment variables):
  AI_META_VERSION   pin a version (e.g. 0.4.0); default: latest release.
  AI_META_BIN_DIR   install directory; default: %LOCALAPPDATA%\ai-meta\bin.
  AI_META_REPO      owner/repo to fetch from; default: tizz98/ai-meta.
#>
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$ProgressPreference = 'SilentlyContinue'

$repo = if ($env:AI_META_REPO) { $env:AI_META_REPO } else { 'tizz98/ai-meta' }
$binDir = if ($env:AI_META_BIN_DIR) { $env:AI_META_BIN_DIR } else { Join-Path $env:LOCALAPPDATA 'ai-meta\bin' }
$version = $env:AI_META_VERSION

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne 'AMD64' -and $arch -ne 'x86') {
  throw "install: unsupported Windows arch $arch (no published build)"
}
$tgt = 'x86_64-pc-windows-msvc'
$ext = '.exe'

if ($version) {
  $version = $version.TrimStart('v')
  $base = "https://github.com/$repo/releases/download/v$version"
  $label = "v$version"
} else {
  $base = "https://github.com/$repo/releases/latest/download"
  $label = 'latest'
}
$asset = "ai-meta-$tgt$ext"
$url = "$base/$asset"

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("ai-meta-install-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
  Write-Host "downloading meta ($label) for $tgt..."
  $dl = Join-Path $tmp $asset
  Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $dl

  # Verify the checksum when the release publishes one (it always should).
  $sumFile = Join-Path $tmp "$asset.sha256"
  $haveSum = $false
  try {
    Invoke-WebRequest -UseBasicParsing -Uri "$url.sha256" -OutFile $sumFile
    $haveSum = $true
  } catch {
    Write-Warning "no checksum published for $asset; skipping verification"
  }
  if ($haveSum) {
    $rawSum = Get-Content -Raw -LiteralPath $sumFile
    $want = if ($rawSum) { (($rawSum.Trim()) -split '\s+')[0].ToLower() } else { '' }
    $have = (Get-FileHash -Algorithm SHA256 -LiteralPath $dl).Hash.ToLower()
    if ($want -ne $have) {
      throw "install: checksum mismatch for $asset (expected $want, got $have)"
    }
  }

  New-Item -ItemType Directory -Force -Path $binDir | Out-Null
  $dest = Join-Path $binDir "meta$ext"
  Move-Item -Force -LiteralPath $dl -Destination $dest
  Write-Host "installed meta -> $dest"
} finally {
  Remove-Item -Recurse -Force -LiteralPath $tmp -ErrorAction SilentlyContinue
}

# Nudge (do not auto-edit PATH) if the install dir isn't on PATH.
if (($env:PATH -split ';') -notcontains $binDir) {
  Write-Host ""
  Write-Host "note: $binDir is not on your PATH. Add it (User scope), then restart your shell:"
  Write-Host "  [Environment]::SetEnvironmentVariable('Path', (`"$binDir;`" + [Environment]::GetEnvironmentVariable('Path','User')), 'User')"
}
Write-Host ""
Write-Host "done. Run 'meta init' in a repo to get started."
