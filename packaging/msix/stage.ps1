# Builds the Store package from the release exe: stages the exe, AppxManifest.xml and logos in -Out, then
# packs <Out>.msix with makeappx from the Windows SDK. The package is unsigned: the Store signs it.
# -Stage stops after staging, for a local test with Add-AppxPackage -Register (needs Developer Mode).
param(
  [Parameter(Mandatory)] [string] $Exe,
  [Parameter(Mandatory)] [string] $Version,   # 1.0.1 becomes the MSIX version 1.0.1.0
  [Parameter(Mandatory)] [string] $IdentityName,
  [Parameter(Mandatory)] [string] $Publisher,
  [Parameter(Mandatory)] [string] $PublisherDisplayName,
  [Parameter(Mandatory)] [string] $Out,
  [switch] $Stage
)
$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
Remove-Item $Out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$Out\Assets" | Out-Null
Copy-Item $Exe "$Out\WallpaperSplitter.exe"
Copy-Item "$here\Assets\*.png" "$Out\Assets"
$esc = { param($s) [System.Security.SecurityElement]::Escape($s) }
(Get-Content "$here\AppxManifest.xml" -Raw).
  Replace("{{IDENTITY_NAME}}", (& $esc $IdentityName)).
  Replace("{{PUBLISHER}}", (& $esc $Publisher)).
  Replace("{{PUBLISHER_DISPLAY_NAME}}", (& $esc $PublisherDisplayName)).
  Replace("{{VERSION}}", "$Version.0") | Set-Content "$Out\AppxManifest.xml" -Encoding utf8
if ($Stage) { return }
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
  Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "makeappx.exe not found: install the Windows SDK" }
& $makeappx.FullName pack /d $Out /p "$Out.msix" /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed ($LASTEXITCODE)" }
