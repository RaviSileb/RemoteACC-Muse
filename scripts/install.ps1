# RemoteACC CLI installer (Windows PowerShell 5.1+)
# Spuštění: irm https://raw.githubusercontent.com/RaviSileb/RemoteACC-Muse/main/scripts/install.ps1 | iex
param(
  [string]$Version = 'latest',
  [string]$InstallDir = (Join-Path $HOME '.local\bin'),
  [switch]$NoPathUpdate
)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo = 'RaviSileb/RemoteACC-Muse'
$Asset = 'racc-cli-windows-x86_64.zip'
if ($Version -eq 'latest') {
  $Url = "https://github.com/$Repo/releases/latest/download/$Asset"
} else {
  $Tag = $Version
  if (-not $Tag.StartsWith('v')) { $Tag = "v$Tag" }
  $Url = "https://github.com/$Repo/releases/download/$Tag/$Asset"
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory $tmp | Out-Null
try {
  $zip = Join-Path $tmp $Asset
  Invoke-WebRequest -Uri $Url -OutFile $zip
  Expand-Archive -Path $zip -DestinationPath $tmp -Force
  New-Item -ItemType Directory $InstallDir -Force | Out-Null
  Copy-Item (Join-Path $tmp 'racc-cli.exe') (Join-Path $InstallDir 'racc-cli.exe') -Force
} finally {
  Remove-Item $tmp -Recurse -Force
}

if (-not $NoPathUpdate) {
  $path = [Environment]::GetEnvironmentVariable('Path', 'User')
  if ($path -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('Path', "$path;$InstallDir", 'User')
  }
  if ($env:Path -notlike "*$InstallDir*") { $env:Path += ";$InstallDir" }
}

& (Join-Path $InstallDir 'racc-cli.exe') --help | Out-Null
Write-Output "racc-cli nainstalovano: $(Join-Path $InstallDir 'racc-cli.exe')"
