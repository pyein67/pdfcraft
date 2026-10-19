# Run in a Visual Studio 2022 Developer PowerShell (Desktop development with C++ + Windows SDK).
[CmdletBinding()]
param([string]$FontDir = "", [switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (!$FontDir) { $FontDir = Join-Path (Split-Path $Repo -Parent) 'craft-fonts-korean' }
function Invoke-Checked([scriptblock]$Command) {
  & $Command
  if ($LASTEXITCODE -ne 0) { throw "Command failed (exit $LASTEXITCODE): $Command" }
}
Push-Location $Repo
try {
  $env:CARGO_BUILD_JOBS = '2'
  $env:CARGO_NET_GIT_FETCH_WITH_CLI = 'true'
  Write-Output '::notice title=Korean build phase::Install Rust 1.95 and the Windows x64 target'
  Invoke-Checked { rustup toolchain install 1.95.0 --profile minimal --component rustfmt --component clippy }
  Invoke-Checked { rustup target add --toolchain 1.95.0 x86_64-pc-windows-msvc }
  # Keep the font input used by 0.8.0 releases, then add Korean without dropping other scripts.
  Write-Output '::notice title=Korean build phase::Prepare and verify the external font input'
  if (!(Test-Path $FontDir)) {
    Invoke-Checked { git clone --no-checkout https://github.com/storytold/craft-fonts $FontDir }
    Invoke-Checked { git -C $FontDir checkout --detach 3d9ab3a132ed0eff457e424226a435a901da30d3 }
  } elseif (!(Test-Path (Join-Path $FontDir 'fonts/manifest.txt'))) {
    throw "Existing FontDir has no fonts/manifest.txt; choose a craft-fonts input or a new directory."
  }
  # Font files remain in the external craft-fonts input; both font and OFL hashes are checked.
  Invoke-Checked { cargo +1.95.0 run --locked --profile low-disk -p xtask -- korean-fonts $FontDir }
  $env:CRAFT_FONTS_DIR = (Resolve-Path $FontDir).Path
  $env:CRAFT_FONTS_REQUIRED = '1'
  $env:PDFCRAFT_KOREAN_REQUIRED = '1'
  $env:PDFCRAFT_REQUIRE_WINRES = '1'
  if (!$SkipTests) {
    Write-Output '::notice title=Korean build phase::Run core PDF and font tests'
    Invoke-Checked { cargo +1.95.0 test --locked --profile low-disk --target x86_64-pc-windows-msvc -p pdfcraft-fonts -p pdfcraft-annot -p pdfcraft-edit -p pdfcraft-render }
    Write-Output '::notice title=Korean build phase::Run automation tests'
    Invoke-Checked { cargo +1.95.0 test --locked --profile low-disk --target x86_64-pc-windows-msvc -p pdfcraft-automation --test automation }
    Write-Output '::notice title=Korean build phase::Run Korean localization, font, language-switch and Text box tests'
    Invoke-Checked { cargo +1.95.0 test --locked --profile low-disk --target x86_64-pc-windows-msvc -p pdfcraft-ui-egui --lib --test fonts --test comments --test control --test print }
  }
  Write-Output '::notice title=Korean build phase::Build the release desktop and CLI executables'
  Invoke-Checked { cargo +1.95.0 build --locked --release --target x86_64-pc-windows-msvc -p pdfcraft -p pdfcraft-cli }
  $TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Repo 'target' }
  $Output = Join-Path $TargetDir 'x86_64-pc-windows-msvc/release'
  Write-Output '::notice title=Korean build phase::Verify native icon resources, registration and GUI PDF launch'
  & (Join-Path $PSScriptRoot 'test-korean-icons.ps1') -Executable (Join-Path $Output 'pdfcraft.exe')
  Write-Output '::notice title=Korean build phase::Package executable attribution and licences'
  Copy-Item (Join-Path $env:CRAFT_FONTS_DIR 'fonts/NanumGothic-Regular.ttf.LICENCE.txt') (Join-Path $Output 'NanumGothic-OFL.txt')
  Copy-Item (Join-Path $env:CRAFT_FONTS_DIR 'KOREAN-ATTRIBUTION.txt') $Output
  Copy-Item (Join-Path $Repo 'NOTICE') $Output
  Copy-Item (Join-Path $PSScriptRoot 'Collect-PrintDiagnostics.ps1') $Output
  Copy-Item (Join-Path $Repo 'docs/windows-print-fix.md') (Join-Path $Output 'WINDOWS-PRINT-FIX.md')
  Copy-Item (Join-Path $Repo 'ATTRIBUTION.md') $Output
  Copy-Item (Join-Path $PSScriptRoot 'Register-PdfCraft.ps1') $Output
  Copy-Item (Join-Path $Repo 'docs/windows-korean-icon.md') (Join-Path $Output 'WINDOWS-ICON-README.md')
  foreach ($LicenceName in @('LICENSE-MIT', 'LICENSE-APACHE')) {
    Copy-Item (Join-Path $Repo $LicenceName) $Output
  }
  # Keep every retained face's licence; never copy font programs into the binary package.
  $Licences = Join-Path $Output 'craft-fonts-licences'
  New-Item -ItemType Directory -Force $Licences | Out-Null
  Get-ChildItem (Join-Path $env:CRAFT_FONTS_DIR 'fonts') -Recurse -File |
    Where-Object { $_.Name -eq 'OFL.txt' -or $_.Name -like '*.LICENCE.txt' } |
    ForEach-Object {
      $Relative = $_.FullName.Substring($env:CRAFT_FONTS_DIR.Length).TrimStart([IO.Path]::DirectorySeparatorChar)
      $Destination = Join-Path $Licences $Relative
      New-Item -ItemType Directory -Force (Split-Path $Destination -Parent) | Out-Null
      Copy-Item $_.FullName $Destination
    }
  if (Test-Path (Join-Path $env:CRAFT_FONTS_DIR 'ATTRIBUTION.md')) {
    Copy-Item (Join-Path $env:CRAFT_FONTS_DIR 'ATTRIBUTION.md') $Licences
  }
  Write-Output "Built PdfCraft 0.8.0: $Output/pdfcraft.exe and pdfcraft-cli.exe"
  Write-Output 'Keep the licence, attribution and NOTICE beside redistributed binaries.'
} finally { Pop-Location }
