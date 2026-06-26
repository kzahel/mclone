param(
  [string]$MinecraftVersion = "1.17.1",
  [string]$OutputRoot = "",
  [string[]]$Sounds = @(
    "minecraft/sounds/damage/fallsmall.ogg",
    "minecraft/sounds/damage/fallbig.ogg"
  )
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$versionRoot = Join-Path $repoRoot "reference\minecraft-$MinecraftVersion"
$versionJson = Join-Path $versionRoot "$MinecraftVersion.json"

if (-not (Test-Path $versionJson)) {
  throw "Missing $versionJson. Run ./scripts/decompile-mc.sh first so the pinned version manifest is present."
}

if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
  $OutputRoot = Join-Path $versionRoot "sound-overlay"
}

$manifest = Get-Content -Raw $versionJson | ConvertFrom-Json
$assetIndex = Invoke-RestMethod -Uri $manifest.assetIndex.url

foreach ($sound in $Sounds) {
  $entry = $assetIndex.objects.PSObject.Properties |
    Where-Object { $_.Name -eq $sound } |
    Select-Object -First 1

  if ($null -eq $entry) {
    throw "Sound $sound was not found in the Minecraft $MinecraftVersion asset index."
  }

  $hash = [string]$entry.Value.hash
  $prefix = $hash.Substring(0, 2)
  $url = "https://resources.download.minecraft.net/$prefix/$hash"
  $target = Join-Path $OutputRoot ("assets/" + $sound)
  $targetDir = Split-Path -Parent $target

  New-Item -ItemType Directory -Force -Path $targetDir | Out-Null

  if (Test-Path $target) {
    $existingHash = (Get-FileHash -Algorithm SHA1 -Path $target).Hash.ToLowerInvariant()
    if ($existingHash -eq $hash) {
      Write-Host "ok $sound"
      continue
    }
  }

  Invoke-WebRequest -Uri $url -OutFile $target
  $actualHash = (Get-FileHash -Algorithm SHA1 -Path $target).Hash.ToLowerInvariant()
  if ($actualHash -ne $hash) {
    Remove-Item -LiteralPath $target -Force
    throw "SHA1 mismatch for $sound; expected $hash, got $actualHash."
  }

  Write-Host "fetched $sound"
}

Write-Host "sound overlay ready: $OutputRoot"
