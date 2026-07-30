# packaging/windows/chocolatey/tools/chocolateyinstall.ps1
# Spec: zerodds-deployment-1.0.md §4.2.3.
$ErrorActionPreference = 'Stop'

$packageName = 'zerodds'
$version     = '1.0.0-rc.7'
$url64       = "https://github.com/zero-objects/zerodds/releases/download/v$version/zerodds-$version-x64.msi"
$checksum64  = '0000000000000000000000000000000000000000000000000000000000000000'

$packageArgs = @{
    packageName    = $packageName
    fileType       = 'msi'
    url64bit       = $url64
    checksum64     = $checksum64
    checksumType64 = 'sha256'
    silentArgs     = "/qn /norestart /l*v `"$($env:TEMP)\$($packageName).MsiInstall.log`""
    validExitCodes = @(0, 3010, 1641)
}

Install-ChocolateyPackage @packageArgs
