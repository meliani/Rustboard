# release.ps1 — bump version, tag, and push to trigger the GitHub release workflow
#
# Usage:
#   .\scripts\release.ps1           # patch bump (default)  0.1.0 → 0.1.1
#   .\scripts\release.ps1 minor     # 0.1.0 → 0.2.0
#   .\scripts\release.ps1 major     # 0.1.0 → 1.0.0
#   .\scripts\release.ps1 1.2.3     # explicit version

param(
    [string]$Bump = "patch"
)

$ErrorActionPreference = "Stop"

function Write-Step { param($m) Write-Host "  $m" -ForegroundColor Cyan }
function Abort      { param($m) Write-Host "ERROR: $m" -ForegroundColor Red; exit 1 }

# ── get current version from git tags ────────────────────────────────────────
$Tags = git tag --list "v*" --sort=-version:refname 2>$null
$Current = if ($Tags) { ($Tags -split "`n")[0].Trim() } else { "v0.0.0" }
$CurrentVer = $Current -replace '^v', ''
$Parts = $CurrentVer -split '\.'
$Major = [int]$Parts[0]; $Minor = [int]$Parts[1]; $Patch = [int]$Parts[2]

# ── calculate next version ────────────────────────────────────────────────────
$NewVer = switch -Regex ($Bump) {
    '^major$'       { $Major++; $Minor = 0; $Patch = 0; "$Major.$Minor.$Patch" }
    '^minor$'       { $Minor++; $Patch = 0; "$Major.$Minor.$Patch" }
    '^patch$'       { $Patch++; "$Major.$Minor.$Patch" }
    '^\d+\.\d+\.\d+$' { $Bump }
    '^v\d+\.\d+\.\d+$' { $Bump -replace '^v', '' }
    default         { Abort "Usage: .\scripts\release.ps1 [major|minor|patch|x.y.z]" }
}

$Tag = "v$NewVer"

# ── guard: must be on main with a clean tree ──────────────────────────────────
$Branch = git rev-parse --abbrev-ref HEAD
if ($Branch -ne "main") { Abort "You must be on 'main' to release (current: $Branch)" }

$Status = git status --porcelain
if ($Status) { Abort "Working tree is dirty. Commit or stash changes first." }

git fetch origin --tags --quiet 2>$null

$Exists = git rev-parse $Tag 2>$null
if ($LASTEXITCODE -eq 0) { Abort "Tag $Tag already exists." }

# ── confirm ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Step "Current : $Current"
Write-Step "Next    : $Tag"
Write-Host ""
$Confirm = Read-Host "Create and push tag $Tag? [y/N]"
if ($Confirm -notmatch '^[Yy]$') { Write-Host "Aborted."; exit 0 }

# ── tag and push ──────────────────────────────────────────────────────────────
git tag $Tag
git push origin $Tag

Write-Host ""
Write-Host "Tag $Tag pushed. GitHub Actions will build and publish the release." -ForegroundColor Green
Write-Host "  https://github.com/meliani/Rustboard/releases/tag/$Tag"
