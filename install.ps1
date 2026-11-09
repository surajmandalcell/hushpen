# Installs the latest Hushpen release for the current Windows user:
#   irm https://hushpen.mandalsuraj.com/install.ps1 | iex
# HUSHPEN_NO_OPEN=1 skips opening the app.
function Install-Hushpen {
    $ErrorActionPreference = 'Stop'
    if (-not [Environment]::Is64BitOperatingSystem -or
        ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64' -and $env:PROCESSOR_ARCHITEW6432 -ne 'AMD64')) {
        throw 'Hushpen needs x64 Windows.'
    }

    $repo = 'surajmandalcell/hushpen'
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
    $tag = $release.tag_name
    if ($tag -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') {
        throw 'Could not find the latest Hushpen release.'
    }
    $version = $tag.Substring(1)
    $name = "Hushpen-$version-windows-x64-setup.exe"
    $base = "https://github.com/$repo/releases/download/$tag"
    $tempRoot = if ($env:TMPDIR) { $env:TMPDIR } else { [IO.Path]::GetTempPath() }
    $temp = Join-Path $tempRoot "hushpen-install-$([Guid]::NewGuid())"
    New-Item -ItemType Directory -Path $temp | Out-Null
    try {
        Write-Host "Downloading $name"
        $setup = Join-Path $temp $name
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$name" -OutFile $setup
        $sums = (Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS").Content
        if ($sums -is [byte[]]) { $sums = [Text.Encoding]::UTF8.GetString($sums) }
        $pattern = '(?m)^([a-fA-F0-9]{64})[ \t]+\*?' + [Regex]::Escape($name) + '\r?$'
        $entries = [Regex]::Matches($sums, $pattern)
        if ($entries.Count -ne 1) {
            throw "$name must appear once in SHA256SUMS. Nothing was installed."
        }
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $setup).Hash -ne $entries[0].Groups[1].Value) {
            throw "$name does not match its checksum. Nothing was installed."
        }
        $process = Start-Process -FilePath $setup -ArgumentList '/S' -PassThru
        if (-not $process.WaitForExit(300000)) {
            throw 'Hushpen setup did not finish within five minutes.'
        }
        if ($process.ExitCode -ne 0) {
            throw "Hushpen setup failed (exit $($process.ExitCode))."
        }
        Write-Host "Installed Hushpen $version."
        if ($env:HUSHPEN_NO_OPEN -ne '1') {
            Start-Process -FilePath (Join-Path $env:LOCALAPPDATA 'Programs\Hushpen\hushpen.exe')
        }
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force
    }
}

if ($MyInvocation.InvocationName -ne '.') { Install-Hushpen }
