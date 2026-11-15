# Script de vérification automatique de cohérence ACL pour Oxygen
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$libRs = Get-Content "$repoRoot\src-tauri\src\lib.rs" -Raw
$buildRs = Get-Content "$repoRoot\src-tauri\build.rs" -Raw
$defaultJson = Get-Content "$repoRoot\src-tauri\capabilities\default.json" -Raw
$remoteJson = Get-Content "$repoRoot\src-tauri\capabilities\ai-remote.json" -Raw

# Extraire les commandes de generate_handler!
if ($libRs -match 'generate_handler!\[([\s\S]*?)\]') {
    $handlerBody = $matches[1]
    $commands = $handlerBody -split ',' | ForEach-Object {
        $_.Trim() -replace '^.*::', ''
    } | Where-Object { $_ -ne '' }
} else {
    throw "Impossible de parser generate_handler! dans lib.rs"
}

Write-Host "Nombre de commandes trouvées dans generate_handler! : $($commands.Count)"

$errors = @()

foreach ($cmd in $commands) {
    $perm = "allow-" + ($cmd -replace '_', '-')

    # Vérifier présence dans build.rs
    if ($buildRs -notmatch "`"$cmd`"") {
        $errors += "build.rs ne déclare pas `"$cmd`""
    }

    # Vérifier permissions default.json vs ai-remote.json
    if ($cmd -eq "ai_push_page_text") {
        if ($defaultJson -match "`"$perm`"") {
            $errors += "SÉCURITÉ : $perm ne doit PAS être dans default.json"
        }
        if ($remoteJson -notmatch "`"$perm`"") {
            $errors += "ai-remote.json doit déclarer `"$perm`""
        }
    } else {
        if ($defaultJson -notmatch "`"$perm`"") {
            $errors += "default.json manque la permission `"$perm`""
        }
        if ($remoteJson -match "`"$perm`"") {
            $errors += "SÉCURITÉ : $perm ne doit JAMAIS être dans ai-remote.json"
        }
    }
}

if ($errors.Count -gt 0) {
    Write-Error ("Erreurs de validation ACL :`n" + ($errors -join "`n"))
    exit 1
}

Write-Host "Validation ACL réussie : toutes les $($commands.Count) commandes sont synchronisées et sécurisées." -ForegroundColor Green
exit 0
