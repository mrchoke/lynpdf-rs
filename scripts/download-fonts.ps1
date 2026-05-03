param(
    [string]$Dest = "fonts",
    [switch]$Force
)

$ErrorActionPreference = "Stop"

if (!(Get-Command Invoke-WebRequest -ErrorAction SilentlyContinue)) {
    throw "Invoke-WebRequest is required"
}

New-Item -ItemType Directory -Path $Dest -Force | Out-Null

$tlwgZipUrl = "https://github.com/tlwg/fonts-tlwg/releases/download/v0.7.3/otf-tlwg-0.7.3.zip"

$fonts = @(
    @{ Name = "Sarabun-Regular.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-Regular.ttf" },
    @{ Name = "Sarabun-Bold.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-Bold.ttf" },
    @{ Name = "Sarabun-Italic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-Italic.ttf" },
    @{ Name = "Sarabun-BoldItalic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/sarabun/Sarabun-BoldItalic.ttf" },
    @{ Name = "Prompt-Regular.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-Regular.ttf" },
    @{ Name = "Prompt-Bold.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-Bold.ttf" },
    @{ Name = "Prompt-Italic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-Italic.ttf" },
    @{ Name = "Prompt-BoldItalic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/prompt/Prompt-BoldItalic.ttf" },
    @{ Name = "Kanit-Regular.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-Regular.ttf" },
    @{ Name = "Kanit-Bold.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-Bold.ttf" },
    @{ Name = "Kanit-Italic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-Italic.ttf" },
    @{ Name = "Kanit-BoldItalic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/kanit/Kanit-BoldItalic.ttf" },
    @{ Name = "Mitr-Regular.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/mitr/Mitr-Regular.ttf" },
    @{ Name = "Mitr-Bold.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/mitr/Mitr-Bold.ttf" },
    @{ Name = "ChakraPetch-Regular.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-Regular.ttf" },
    @{ Name = "ChakraPetch-Bold.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-Bold.ttf" },
    @{ Name = "ChakraPetch-Italic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-Italic.ttf" },
    @{ Name = "ChakraPetch-BoldItalic.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/chakrapetch/ChakraPetch-BoldItalic.ttf" },
    @{ Name = "IBMPlexSans-Variable.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/ibmplexsans/IBMPlexSans%5Bwdth%2Cwght%5D.ttf" },
    @{ Name = "InterVariable.ttf"; Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/inter/Inter%5Bopsz%2Cwght%5D.ttf" },
    @{ Name = "NotoColorEmoji.ttf"; Url = "https://github.com/googlefonts/noto-emoji/raw/main/fonts/NotoColorEmoji.ttf" }
)

foreach ($font in $fonts) {
    $target = Join-Path $Dest $font.Name
    if ((Test-Path $target) -and -not $Force.IsPresent) {
        Write-Host "skip  $($font.Name)"
        continue
    }

    Write-Host "fetch $($font.Name)"
    Invoke-WebRequest -Uri $font.Url -OutFile $target
}

$tlwgDest = Join-Path $Dest "tlwg/otf"
New-Item -ItemType Directory -Path $tlwgDest -Force | Out-Null

$existingTlwgFonts = Get-ChildItem -Path $tlwgDest -Filter "*.otf" -File -ErrorAction SilentlyContinue
if ($existingTlwgFonts -and -not $Force.IsPresent) {
    Write-Host "skip  tlwg-otf bundle"
}
else {
    $tempRoot = [System.IO.Path]::GetTempPath()
    $zipPath = Join-Path $tempRoot ("otf-tlwg-" + [System.Guid]::NewGuid().ToString() + ".zip")
    $extractDir = Join-Path $tempRoot ("lynpdf-rs-tlwg-" + [System.Guid]::NewGuid().ToString())

    Write-Host "fetch tlwg-otf bundle"
    Invoke-WebRequest -Uri $tlwgZipUrl -OutFile $zipPath

    New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    $otfFiles = Get-ChildItem -Path $extractDir -Recurse -Filter "*.otf" -File
    if (-not $otfFiles) {
        Remove-Item -Path $zipPath -Force -ErrorAction SilentlyContinue
        Remove-Item -Path $extractDir -Recurse -Force -ErrorAction SilentlyContinue
        throw "TLWG OTF bundle downloaded but no .otf files were found"
    }

    foreach ($otf in $otfFiles) {
        Copy-Item -Path $otf.FullName -Destination (Join-Path $tlwgDest $otf.Name) -Force
    }

    Remove-Item -Path $zipPath -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $extractDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "done  tlwg-otf ($($otfFiles.Count) files)"
}

Write-Host "done: fonts downloaded to $Dest"
