# packaging/windows/chocolatey/tools/chocolateyuninstall.ps1
$ErrorActionPreference = 'Stop'
$packageArgs = @{
    packageName  = 'zerodds'
    softwareName = 'ZeroDDS*'
    fileType     = 'msi'
    silentArgs   = '/qn /norestart'
    validExitCodes = @(0, 1605, 1614, 1641, 3010)
}
Uninstall-ChocolateyPackage @packageArgs
