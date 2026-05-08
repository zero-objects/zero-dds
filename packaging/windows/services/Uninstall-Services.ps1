# packaging/windows/services/Uninstall-Services.ps1
# Stops and removes all ZeroDDS bridge services.
# Spec: zerodds-deployment-1.0.md §4.3.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'

$Services = @(
    'ZeroDDSWSBridge', 'ZeroDDSMQTTBridge', 'ZeroDDSCoAPBridge',
    'ZeroDDSAMQPBridge', 'ZeroDDSGrpcBridge', 'ZeroDDSCORBABridge'
)

foreach ($s in $Services) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        Write-Host "Stopping + removing $s"
        sc.exe stop  $s | Out-Null
        sc.exe delete $s | Out-Null
    }
}

Write-Host "ZeroDDS services removed."
