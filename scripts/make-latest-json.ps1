# Генерирует latest.json для автообновления Tauri.
#
# Использование (из корня проекта):
#   .\scripts\make-latest-json.ps1 -Version 0.3.0 -Notes "Что нового"
#
# Читает подпись из собранного .sig файла и кладёт latest.json рядом с установщиками.

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Notes = "",
    [string]$Repo = "dkisnotokay/omnichat89"
)

$ErrorActionPreference = "Stop"

$bundleDir = Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle"
$setupName = "Omnichat89_${Version}_x64-setup.exe"
$sigPath = Join-Path $bundleDir "nsis\$setupName.sig"
$outPath = Join-Path $bundleDir "latest.json"

if (-not (Test-Path $sigPath)) {
    Write-Error @"
Не найден файл подписи: $sigPath
Собирайте релиз с ключом:
  `$env:TAURI_SIGNING_PRIVATE_KEY = "`$env:USERPROFILE\.tauri\omnichat89.key"
  `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
  npm run tauri build
"@
}

$signature = (Get-Content $sigPath -Raw).Trim()

$manifest = [ordered]@{
    version   = $Version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $signature
            url       = "https://github.com/$Repo/releases/download/v$Version/$setupName"
        }
    }
}

$json = $manifest | ConvertTo-Json -Depth 5
# Без BOM — иначе updater не разберёт JSON
[System.IO.File]::WriteAllText($outPath, $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Output "latest.json создан: $outPath"
Write-Output "  version:   $Version"
Write-Output "  url:       https://github.com/$Repo/releases/download/v$Version/$setupName"
Write-Output "  signature: $($signature.Substring(0, [Math]::Min(40, $signature.Length)))..."
