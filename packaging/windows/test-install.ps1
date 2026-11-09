$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/../../install.ps1"

$script:setupBytes = [Text.Encoding]::UTF8.GetBytes('verified setup')
$sha = [Security.Cryptography.SHA256]::Create()
try { $script:setupHash = ([BitConverter]::ToString($sha.ComputeHash($script:setupBytes))).Replace('-', '') }
finally { $sha.Dispose() }
$script:checksum = $script:setupHash
$script:entries = 1
$script:tag = 'v2.4.0'
$script:stableWindows = $true
$script:releases = @()
$script:listCalls = 0
$script:exitCode = 0
$script:installs = 0
$script:launches = 0
$script:downloads = @()
$env:HUSHPEN_NO_OPEN = '1'

function Invoke-RestMethod {
    param($Uri, $Headers, $TimeoutSec)
    if ($Uri.EndsWith('/latest')) {
        return @{ tag_name = $script:tag; assets = @($(if ($script:stableWindows) { @{ name = "Hushpen-$($script:tag.Substring(1))-windows-x64-setup.exe" } })) }
    }
    if (-not $Uri.EndsWith('/releases?per_page=20')) { throw 'Wrong releases URL.' }
    $script:listCalls++
    $script:releases
}
function Invoke-WebRequest {
    param($Uri, $OutFile, [switch]$UseBasicParsing)
    if ($OutFile) {
        $script:downloads += $OutFile
        [IO.File]::WriteAllBytes($OutFile, $script:setupBytes)
    } else {
        @{ Content = ("$script:checksum  Hushpen-$($script:tag.Substring(1))-windows-x64-setup.exe`r`n" * $script:entries) }
    }
}
function Start-Process {
    param($FilePath, $ArgumentList, [switch]$PassThru)
    if ($FilePath.EndsWith('Hushpen.lnk')) { $script:launches++; return }
    if ($ArgumentList -ne '/S' -or -not $FilePath.EndsWith('-setup.exe')) { throw 'Wrong setup command.' }
    $script:installs++
    $process = [PSCustomObject]@{ ExitValue = $script:exitCode; HasHandle = $false }
    $process | Add-Member -MemberType ScriptProperty -Name Handle -Value { $this.HasHandle = $true; 1 }
    $process | Add-Member -MemberType ScriptProperty -Name ExitCode -Value {
        if (-not $this.HasHandle) { throw 'The process handle was not retained.' }
        $this.ExitValue
    }
    $process | Add-Member -MemberType ScriptMethod -Name WaitForExit -Value { param($Timeout) $true }
    $process
}
function Expect-Failure {
    param($Message)
    $before = $script:installs
    try { Install-Hushpen; throw 'Expected a failure.' }
    catch { if ($_.Exception.Message -notlike "*$Message*") { throw } }
    if ($script:installs -ne $before) { throw 'Unverified setup was started.' }
}

Install-Hushpen
if ($script:installs -ne 1) { throw 'Verified setup did not run.' }
if ($script:listCalls -ne 0) { throw 'A stable Windows release must take priority.' }
$script:stableWindows = $false
$script:tag = 'v2.5.0-rc.2'
$script:releases = @(
    @{ tag_name = 'v2.5.0-rc.1'; published_at = '2026-11-07T00:00:00Z'; assets = @(@{ name = 'Hushpen-2.5.0-rc.1-windows-x64-setup.exe' }) },
    @{ tag_name = 'v2.6.0'; published_at = '2026-11-09T00:00:00Z'; draft = $true; assets = @(@{ name = 'Hushpen-2.6.0-windows-x64-setup.exe' }) },
    @{ tag_name = 'v2.6.1'; published_at = '2026-11-09T00:00:00Z'; assets = @(@{ name = 'Hushpen-2.6.1-macos-arm64.dmg' }) },
    @{ tag_name = $script:tag; published_at = '2026-11-08T00:00:00Z'; prerelease = $true; assets = @(@{ name = 'Hushpen-2.5.0-rc.2-windows-x64-setup.exe' }) }
)
Install-Hushpen
if (-not $script:downloads[-1].EndsWith('Hushpen-2.5.0-rc.2-windows-x64-setup.exe')) { throw 'The newest public Windows preview must be selected.' }
$script:releases = @()
Expect-Failure 'latest Hushpen release'
$script:stableWindows = $true
$script:tag = 'v2.4.0'
$env:HUSHPEN_NO_OPEN = ''
Install-Hushpen
if ($script:launches -ne 1) { throw 'Setup must open its installed shortcut.' }
$env:HUSHPEN_NO_OPEN = '1'
$script:checksum = '0' * 64
Expect-Failure 'does not match'
$script:checksum = $script:setupHash
$script:entries = 0
Expect-Failure 'must appear once'
$script:entries = 2
Expect-Failure 'must appear once'
$script:entries = 1
$script:tag = 'v2.4.0/evil'
Expect-Failure 'latest Hushpen release'
$script:tag = 'v2.4.0'
$script:exitCode = 7
try { Install-Hushpen; throw 'Expected setup failure.' }
catch { if ($_.Exception.Message -notlike '*setup failed (exit 7)*') { throw } }
foreach ($download in $script:downloads) {
    if (Test-Path -LiteralPath (Split-Path $download)) { throw 'Temporary download was not removed.' }
}
Write-Host 'PowerShell installer checks passed.'
