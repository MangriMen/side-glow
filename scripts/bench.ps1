<#
.SYNOPSIS
Measures CPU and GPU load of SideGlow builds against a full-screen test pattern.

.DESCRIPTION
For every build and every pattern mode (static, slow, fast) this starts the test
pattern on the primary monitor, starts SideGlow, waits for it to settle and then
samples its CPU time and 3D GPU engine utilization. Don't use the computer while it
runs: anything else on screen changes what is being captured.

.EXAMPLE
cargo build --release --examples
./scripts/bench.ps1 -Builds @{ current = 'target/release/SideGlow.exe' }

.EXAMPLE
./scripts/bench.ps1 -Builds ([ordered]@{ old = 'old/SideGlow.exe'; new = 'target/release/SideGlow.exe' }) -Rounds 2
#>
param(
    [System.Collections.IDictionary] $Builds = [ordered]@{ current = 'target/release/SideGlow.exe' },
    [string] $Pattern = 'target/release/examples/test_pattern.exe',
    [string[]] $Modes = @('static', 'slow', 'fast'),
    [int] $Seconds = 12,
    [int] $Warmup = 5,
    [int] $Rounds = 1
)

$ErrorActionPreference = 'Stop'

function Measure-Run([string] $exe, [string] $mode) {
    $pattern = Start-Process -FilePath $Pattern -ArgumentList $mode -PassThru
    Start-Sleep -Seconds 2
    $app = Start-Process -FilePath $exe -PassThru
    try {
        Start-Sleep -Seconds $Warmup
        $proc = Get-Process -Id $app.Id
        $cpuBefore = $proc.TotalProcessorTime
        # Sampling the GPU counter once per second doubles as the measurement window.
        $gpuPath = "\GPU Engine(pid_$($app.Id)_*engtype_3D)\Utilization Percentage"
        $samples = Get-Counter -Counter $gpuPath -SampleInterval 1 -MaxSamples $Seconds -ErrorAction SilentlyContinue
        $proc.Refresh()
        $cpu = ($proc.TotalProcessorTime - $cpuBefore).TotalSeconds / $Seconds * 100
        # Each sample has one value per engine instance; sum them per sample, then average.
        $gpu = if ($samples) {
            ($samples | ForEach-Object { ($_.CounterSamples | Measure-Object CookedValue -Sum).Sum } |
                Measure-Object -Average).Average
        } else { [double]::NaN }
        [pscustomobject]@{
            CpuPercent = [math]::Round($cpu, 1)
            GpuPercent = [math]::Round($gpu, 1)
            WorkingSetMB = [math]::Round($proc.WorkingSet64 / 1MB)
        }
    } finally {
        Stop-Process -Id $app.Id -ErrorAction SilentlyContinue
        Stop-Process -Id $pattern.Id -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
}

foreach ($path in @($Pattern) + @($Builds.Values)) {
    if (-not (Test-Path $path)) { throw "not found: $path (run cargo build --release --examples)" }
}

$results = foreach ($round in 1..$Rounds) {
    foreach ($mode in $Modes) {
        # Alternate the build order between rounds to spread out drift in background load.
        $names = @($Builds.Keys)
        if ($round % 2 -eq 0) { [array]::Reverse($names) }
        foreach ($name in $names) {
            Write-Host "round $round, $mode, $name..."
            $run = Measure-Run $Builds[$name] $mode
            [pscustomobject]@{
                Build = $name; Mode = $mode; Round = $round
                CpuPercent = $run.CpuPercent; GpuPercent = $run.GpuPercent; WorkingSetMB = $run.WorkingSetMB
            }
        }
    }
}

$results | Format-Table -AutoSize
$results | Group-Object Build, Mode | ForEach-Object {
    [pscustomobject]@{
        Build = $_.Group[0].Build
        Mode = $_.Group[0].Mode
        'CPU % (one core)' = [math]::Round(($_.Group | Measure-Object CpuPercent -Average).Average, 1)
        'GPU 3D %' = [math]::Round(($_.Group | Measure-Object GpuPercent -Average).Average, 1)
    }
} | Sort-Object Mode, Build | Format-Table -AutoSize
