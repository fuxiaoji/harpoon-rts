param(
    [int]$Port = 8795,
    [string]$Scenario = '',
    [switch]$NoBrowser
)
$ErrorActionPreference = 'Stop'
$sourceRoot = Split-Path -Parent $PSScriptRoot
$store = if (Test-Path -LiteralPath 'E:\') { 'E:\CodexBuilds\harpoon-academy' } else { Join-Path $env:LOCALAPPDATA 'HarpoonRTS' }
$runtimeDir = Join-Path $store 'launcher'
New-Item -ItemType Directory -Force -Path $runtimeDir,(Join-Path $store 'temp') | Out-Null
$url = "http://127.0.0.1:$Port/"
try {
    $health = Invoke-RestMethod ($url+'api/health') -TimeoutSec 2
    if ($health) {
        Write-Host "Game already running: $url (current battle preserved)"
        if (!$NoBrowser) { Start-Process $url }
        return
    }
} catch { }
Push-Location -LiteralPath $sourceRoot
try {
    $cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (!(Test-Path -LiteralPath $cargo)) { throw 'Rust Cargo is required for the first launch. See README.' }
    $env:CARGO_TARGET_DIR = Join-Path $store 'target'
    $env:TEMP = Join-Path $store 'temp'
    $env:TMP = $env:TEMP
    $python313 = Join-Path $env:LOCALAPPDATA 'Programs\Python\Python313\python.exe'
    if (Test-Path -LiteralPath $python313) { $env:PYO3_PYTHON = $python313 }
    Write-Host 'Checking/building the current server; build cache is outside the source folder...'
    # Windows PowerShell 5.1 在 Stop + *> 下把 cargo 的 stderr 进度当终止错误；构建期间临时放宽，成败仍看退出码
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $cargo build --release -p harpoon-server --bin harpoon-server --locked *> (Join-Path $runtimeDir 'build.log')
    $ErrorActionPreference = $prevEap
    if ($LASTEXITCODE -ne 0) { throw "Build failed. See $runtimeDir\build.log" }
    $builtServer = Join-Path $env:CARGO_TARGET_DIR 'release\harpoon-server.exe'
    $binaryHash = (Get-FileHash -LiteralPath $builtServer -Algorithm SHA256).Hash.Substring(0,12)
    $serverPath = Join-Path $runtimeDir "server-$binaryHash.exe"
    if (!(Test-Path -LiteralPath $serverPath)) { Copy-Item -LiteralPath $builtServer -Destination $serverPath }
    if (!$Scenario) {
        $Scenario = if (Test-Path -LiteralPath 'scenarios\dual_carrier_denial.json') { 'dual_carrier_denial' } else { 'tutorial_01' }
    }
    $env:HARPOON_TRAINING_PROGRESS = Join-Path $store 'user-progress.json'
    $serverArgs = @('--port',"$Port",'--web','web','--scenarios','scenarios','--scenario',$Scenario,'--speed','0')
    $serverProcess = Start-Process -FilePath $serverPath -ArgumentList $serverArgs -WorkingDirectory $sourceRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $runtimeDir "server-$Port.out.log") -RedirectStandardError (Join-Path $runtimeDir "server-$Port.err.log")
    @{pid=$serverProcess.Id;port=$Port;source=$sourceRoot;scenario=$Scenario;binary=$binaryHash;started=(Get-Date -Format o)} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $runtimeDir "running-$Port.json") -Encoding UTF8
    $ready = $false
    for ($attempt=0;$attempt -lt 40;$attempt++) {
        Start-Sleep -Milliseconds 250
        try { $null=Invoke-RestMethod ($url+'api/health') -TimeoutSec 1; $ready=$true; break } catch { }
        if ($serverProcess.HasExited) { break }
    }
    if (!$ready) { throw "Server did not start. See $runtimeDir\server.err.log" }
    $startedState = Invoke-RestMethod ($url+'api/state?side=Blue') -TimeoutSec 3
    if ($startedState.scenario_id -ne $Scenario) { throw "The requested scenario did not load. See $runtimeDir\server-$Port.err.log" }
    Write-Host "Ready and paused: $url"
    if (!$NoBrowser) { Start-Process $url }
} finally { Pop-Location }
