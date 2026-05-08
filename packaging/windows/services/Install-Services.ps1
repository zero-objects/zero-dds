# packaging/windows/services/Install-Services.ps1
# Registers all 7 ZeroDDS bridge daemons as Windows services.
# Spec: zerodds-deployment-1.0.md §4.3 (Win-Service).
# Run as Administrator.
[CmdletBinding()]
param(
    [string] $InstallDir = "$env:PROGRAMFILES\ZeroDDS",
    [string] $ConfigDir  = "$env:PROGRAMDATA\ZeroDDS"
)
$ErrorActionPreference = 'Stop'

if (-not ([Security.Principal.WindowsPrincipal] `
          [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
          [Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw "Install-Services.ps1 must be run as Administrator."
}

# Spec §1.1 daemon set.
$Daemons = @(
    @{ Name = 'ZeroDDSWSBridge';    Bin = 'zerodds-ws-bridged.exe';    Cfg = 'ws-bridged.yaml';    Display = 'ZeroDDS WebSocket Bridge' },
    @{ Name = 'ZeroDDSMQTTBridge';  Bin = 'zerodds-mqtt-bridged.exe';  Cfg = 'mqtt-bridged.yaml';  Display = 'ZeroDDS MQTT 5 Bridge' },
    @{ Name = 'ZeroDDSCoAPBridge';  Bin = 'zerodds-coap-bridged.exe';  Cfg = 'coap-bridged.yaml';  Display = 'ZeroDDS CoAP Bridge' },
    @{ Name = 'ZeroDDSAMQPBridge';  Bin = 'zerodds-amqp-bridged.exe';  Cfg = 'amqp-bridged.yaml';  Display = 'ZeroDDS AMQP 1.0 Bridge' },
    @{ Name = 'ZeroDDSGrpcBridge';  Bin = 'zerodds-grpc-bridged.exe';  Cfg = 'grpc-bridged.yaml';  Display = 'ZeroDDS gRPC Bridge' },
    @{ Name = 'ZeroDDSCORBABridge'; Bin = 'zerodds-corba-bridged.exe'; Cfg = 'corba-bridged.yaml'; Display = 'ZeroDDS CORBA Bridge' }
    # ros2-shim ist Diagnose-Tool, kein Service.
)

foreach ($d in $Daemons) {
    $exe = Join-Path $InstallDir "bin\$($d.Bin)"
    $cfg = Join-Path $ConfigDir   $d.Cfg

    if (-not (Test-Path $exe)) {
        Write-Warning "Skip $($d.Name): $exe missing."
        continue
    }

    $existing = Get-Service -Name $d.Name -ErrorAction SilentlyContinue
    if ($existing) {
        Write-Host "Service $($d.Name) already present — stopping + removing first."
        sc.exe stop  $d.Name | Out-Null
        sc.exe delete $d.Name | Out-Null
    }

    $binPath = "`"$exe`" --config `"$cfg`""

    sc.exe create $d.Name `
        binPath=     $binPath `
        start=       auto `
        obj=         "NT SERVICE\$($d.Name)" `
        DisplayName= $d.Display | Out-Null

    sc.exe description $d.Name $d.Display | Out-Null
    sc.exe failure $d.Name reset= 60 actions= restart/5000/restart/10000/restart/60000 | Out-Null

    Write-Host "Registered $($d.Name) -> $exe (config: $cfg)"
}

Write-Host ""
Write-Host "All 6 ZeroDDS bridge services registered. Start with:"
Write-Host "  sc.exe start ZeroDDSWSBridge"
