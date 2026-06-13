$ErrorActionPreference = "Stop"

$Root = $PSScriptRoot
$Dist = Join-Path $Root "dist"
$Release = Join-Path $Root "target\release"

Write-Host "Building release..."
Push-Location $Root
try {
    cargo build --release -p kdrover-gui -p kdrover-dll -p kdrover-cli
} finally {
    Pop-Location
}

$files = @(
    @{ Source = "kdrover.exe"; Required = $true },
    @{ Source = "kdrover_payload.dll"; Required = $true },
    @{ Source = "kdrover-cli.exe"; Required = $false },
    @{ Source = "drover.ini.example"; Required = $false; FromRoot = $true }
)

if (Test-Path $Dist) {
    Get-ChildItem $Dist -Force | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
} else {
    New-Item -ItemType Directory -Path $Dist | Out-Null
}

foreach ($file in $files) {
    $base = if ($file.FromRoot) { $Root } else { $Release }
    $source = Join-Path $base $file.Source
    if (-not (Test-Path $source)) {
        if ($file.Required) {
            throw "Missing build artifact: $source"
        }
        continue
    }
    Copy-Item $source (Join-Path $Dist $file.Source)
}

# Never ship a stray version.dll — it breaks the installer if placed nearby.
$stray = Join-Path $Dist "version.dll"
if (Test-Path $stray) {
    Remove-Item $stray -Force
}

Write-Host ""
Write-Host "Distribution ready:"
Get-ChildItem $Dist | ForEach-Object { Write-Host "  $($_.Name)" }
Write-Host ""
Write-Host "Run: $Dist\kdrover.exe"
