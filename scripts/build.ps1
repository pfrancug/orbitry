# Builds the Orbitry release exe: UI first (embedded into the binary), then Rust.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

# rustup adds ~/.cargo/bin to PATH only for new terminals; fall back to it.
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
  if (Test-Path (Join-Path $cargoBin 'cargo.exe')) { $env:PATH = "$cargoBin;$env:PATH" }
  else { throw 'cargo not found - install Rust from https://rustup.rs' }
}

Push-Location (Join-Path $root 'ui')
try {
  if (-not (Test-Path 'node_modules')) { npm install; if ($LASTEXITCODE) { throw 'npm install failed' } }
  npm run build; if ($LASTEXITCODE) { throw 'UI build failed' }
} finally { Pop-Location }

Push-Location $root
try {
  cargo build --release -p orbitry; if ($LASTEXITCODE) { throw 'cargo build failed' }
  New-Item -ItemType Directory -Force -Path 'dist' | Out-Null
  Copy-Item 'target\release\orbitry.exe' 'dist\orbitry.exe' -Force
  Write-Host "Built dist\orbitry.exe"
} finally { Pop-Location }
