# RemoteACC GUI instalator (Windows).
# Spusteni z korene repa: powershell -ExecutionPolicy Bypass -File scripts\install-gui.ps1
# Predtim: flutter build windows --release (v clients\racc_app) + racc_core.dll vedle exe.
param(
  [string]$AdminPassword = '',
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\racc_app'),
  [string]$SourceDir = ''
)
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent
if ([string]::IsNullOrEmpty($SourceDir)) {
  $SourceDir = Join-Path $repoRoot 'clients\racc_app\build\windows\x64\runner\Release'
}
$exe = Join-Path $SourceDir 'racc_app.exe'
if (-not (Test-Path $exe)) {
  throw "Nenalezeno: $exe - nejdriv spust 'flutter build windows --release'"
}
$dll = Join-Path $SourceDir 'racc_core.dll'
if (-not (Test-Path $dll)) {
  throw "Nenalezeno: $dll - zkopiruj target\release\racc_core.dll vedle exe"
}

# Heslo: zadane, nebo vygenerovane (20 znaku A-Za-z0-9).
if ([string]::IsNullOrEmpty($AdminPassword)) {
  $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
  $alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  $b = New-Object byte[] 20
  $rng.GetBytes($b)
  $AdminPassword = -join ($b | ForEach-Object { $alphabet[$_ % 62] })
}

# Sul (16 B hex) + hash SHA-256("sul:heslo") - stejny postup jako Dart auth.dart.
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
$sb = New-Object byte[] 16
$rng.GetBytes($sb)
$salt = -join ($sb | ForEach-Object { $_.ToString('x2') })
$sha = [Security.Cryptography.SHA256]::Create()
$hash = -join ($sha.ComputeHash(
    [Text.Encoding]::UTF8.GetBytes("${salt}:${AdminPassword}")) |
  ForEach-Object { $_.ToString('x2') })

# Instalace souboru.
New-Item -ItemType Directory $InstallDir -Force | Out-Null
Copy-Item (Join-Path $SourceDir '*') $InstallDir -Recurse -Force

# Admin ucet (JSON bez BOM).
$cfgDir = Join-Path $env:APPDATA 'RemoteACC'
New-Item -ItemType Directory $cfgDir -Force | Out-Null
$adminJson = Join-Path $cfgDir 'admin.json'
$json = (@{ username = 'admin'; salt = $salt; hash = $hash } | ConvertTo-Json -Compress)
[IO.File]::WriteAllText($adminJson, $json, (New-Object Text.UTF8Encoding $false))

# Zastupce v nabidce Start.
$shell = New-Object -ComObject WScript.Shell
$lnk = $shell.CreateShortcut(
    (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\RemoteACC.lnk'))
$lnk.TargetPath = Join-Path $InstallDir 'racc_app.exe'
$lnk.WorkingDirectory = $InstallDir
$lnk.Save()

Write-Output "GUI nainstalovano: $InstallDir\racc_app.exe"
Write-Output "Admin ucet: admin"
Write-Output "Admin heslo: $AdminPassword"
Write-Output "Konfigurace: $adminJson"
