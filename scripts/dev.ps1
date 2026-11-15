param(
  [string]$Root,
  [int]$Port = 4280
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
if (-not $Root) { $Root = Split-Path $repo -Parent }

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargoBin) { $env:Path = "$cargoBin;$env:Path" }

Write-Host "Orbitry API on http://127.0.0.1:$Port"
Write-Host "Orbitry UI  on http://127.0.0.1:4281"
Write-Host "Scanning    $Root"

$api = Start-Process -FilePath 'cargo' -ArgumentList @('run','-p','orbitry','--','--root',$Root,'--port',"$Port",'--no-open') -WorkingDirectory $repo -PassThru -NoNewWindow
Push-Location (Join-Path $repo 'ui')
try {
  npm run dev
} finally {
  Pop-Location
  if ($api -and -not $api.HasExited) {
    Stop-Process -Id $api.Id -Force -ErrorAction SilentlyContinue
  }
}