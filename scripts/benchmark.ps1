param(
    [Parameter(Mandatory)][string]$Baseline,
    [string]$Candidate = "$PSScriptRoot/../target/release/earcontrol.exe",
    [ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$BaselineName = 'tauri_2_1_2',
    [ValidatePattern('^[a-zA-Z0-9_-]+$')][string]$CandidateName = 'rust_3_0_0',
    [switch]$BaselineUsesIsolatedSettings,
    [ValidateRange(3,30)][int]$Runs = 5,
    [ValidateRange(1,30)][int]$SettleSeconds = 3,
    [ValidateRange(1,30)][int]$IdleSeconds = 3,
    [string]$Output = "$PSScriptRoot/../docs/benchmark-results/processes.json"
)
$ErrorActionPreference = 'Stop'
$baselinePath = (Resolve-Path -LiteralPath $Baseline).Path
$candidatePath = (Resolve-Path -LiteralPath $Candidate).Path
if (Get-Process earcontrol -ErrorAction SilentlyContinue) { throw 'Close running EarControl copies before benchmarking. No existing process will be terminated.' }
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class EarControlBenchmarkWindow {
    private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder title, int maxCount);

    public static bool IsMainWindowVisible(int processId) {
        bool found = false;
        EnumWindows((hwnd, parameter) => {
            uint owner;
            GetWindowThreadProcessId(hwnd, out owner);
            if (owner != processId || !IsWindowVisible(hwnd)) return true;
            var title = new StringBuilder(256);
            GetWindowText(hwnd, title, title.Capacity);
            string name = title.ToString();
            found = name == "EarControl" || (name.StartsWith("EarControl \u00b7 ", StringComparison.Ordinal)
                && name.IndexOf("practice", StringComparison.OrdinalIgnoreCase) < 0);
            return !found;
        }, IntPtr.Zero);
        return found;
    }
}
'@
$scratch = Join-Path ([IO.Path]::GetTempPath()) ("earcontrol-bench-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$baselineSettings = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'app.earcontrol.desktop/profiles.json'
$candidateSettings = Join-Path $scratch 'candidate-profiles.json'
if (Test-Path -LiteralPath $baselineSettings) {
    Copy-Item -LiteralPath $baselineSettings -Destination $candidateSettings
    $settingsMethod = 'Both apps read the same existing configuration; candidate uses an isolated copy. Baseline only reads its original file.'
    if ($BaselineUsesIsolatedSettings) {
        $settingsMethod = 'Both native Rust apps read the same isolated copy of the existing configuration. Neither app writes the original file.'
    }
} else {
    $settingsMethod = 'No baseline configuration exists; both apps use their unchanged version-2 defaults.'
}

function Read-ProcessTree([int]$RootId) {
    $processes = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId)
    $ids = [Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootId)
    do {
        $added = $false
        foreach ($item in $processes) { if ($ids.Contains([int]$item.ParentProcessId)) { if ($ids.Add([int]$item.ProcessId)) { $added = $true } } }
    } while ($added)
    $snapshots = foreach ($processId in $ids) {
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if ($process) { [pscustomobject]@{ id=$processId; start_ticks=$process.StartTime.ToUniversalTime().Ticks; private_bytes=$process.PrivateMemorySize64; working_set_bytes=$process.WorkingSet64; cpu_ms=$process.TotalProcessorTime.TotalMilliseconds } }
    }
    return @($snapshots)
}
function Measure-Launch([string]$Name, [string]$Path, [int]$Round) {
    $startInfo = [Diagnostics.ProcessStartInfo]::new($Path)
    $startInfo.UseShellExecute = $false
    $startInfo.WorkingDirectory = Split-Path -Parent $Path
    $startInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $startInfo.Environment['EARCONTROL_SETTINGS_PATH'] = $candidateSettings
    $startInfo.Environment['WEBVIEW2_USER_DATA_FOLDER'] = Join-Path $scratch "$Name/webview"
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $visible = $false
        do {
            if ($process.HasExited) { throw "$Name exited before its window became visible." }
            $visible = [EarControlBenchmarkWindow]::IsMainWindowVisible($process.Id)
            if ($visible) { break }
            Start-Sleep -Milliseconds 10
        } while ($watch.Elapsed.TotalSeconds -lt 30)
        if (!$visible) { throw "$Name did not create a visible EarControl main window within 30 seconds." }
        $windowMs = $watch.Elapsed.TotalMilliseconds
        Start-Sleep -Seconds $SettleSeconds
        $initial = @(Read-ProcessTree $process.Id)
        $idleWatch = [Diagnostics.Stopwatch]::StartNew()
        Start-Sleep -Seconds $IdleSeconds
        $final = @(Read-ProcessTree $process.Id)
        $idleElapsedMs = $idleWatch.Elapsed.TotalMilliseconds
        $initialIdentities = @($initial | ForEach-Object { "$($_.id):$($_.start_ticks)" } | Sort-Object)
        $finalIdentities = @($final | ForEach-Object { "$($_.id):$($_.start_ticks)" } | Sort-Object)
        if (Compare-Object $initialIdentities $finalIdentities) { throw "$Name process tree changed during idle sampling. Increase SettleSeconds and rerun; CPU would otherwise omit short-lived processes." }
        $cpuMs = 0.0
        foreach ($item in $final) {
            $previous = $initial | Where-Object id -eq $item.id | Select-Object -First 1
            if ($previous) { $cpuMs += [Math]::Max(0, $item.cpu_ms - $previous.cpu_ms) }
        }
        [pscustomobject]@{
            app=$Name; round=$Round; window_ms=[Math]::Round($windowMs,3)
            processes=$final.Count; private_bytes=($final | Measure-Object private_bytes -Sum).Sum
            working_set_bytes=($final | Measure-Object working_set_bytes -Sum).Sum
            idle_cpu_ms=[Math]::Round($cpuMs,3); idle_observation_ms=[Math]::Round($idleElapsedMs,3)
            idle_cpu_one_core_pct=[Math]::Round(100*$cpuMs/$idleElapsedMs,4)
        }
    } finally {
        # Remember exact process lifetimes before the parent exits. Browser
        # children can outlive a normal parent exit and must not leak into runs.
        $owned = @(Read-ProcessTree $process.Id)
        if (!$process.HasExited) {
            [void]$process.CloseMainWindow()
            if (!$process.WaitForExit(5000)) { $process.Kill($true); $process.WaitForExit() }
        }
        foreach ($item in $owned) {
            $remaining = Get-Process -Id $item.id -ErrorAction SilentlyContinue
            if ($remaining -and $remaining.StartTime.ToUniversalTime().Ticks -eq $item.start_ticks) {
                $remaining.Kill()
                if (!$remaining.WaitForExit(5000)) { throw 'A benchmark descendant did not exit; stop before contaminating later runs.' }
            }
        }
        $process.Dispose()
    }
}

$samples = @()
foreach ($item in @(@($BaselineName,$baselinePath), @($CandidateName,$candidatePath))) {
    Write-Host "Warming $($item[0])"
    $null = Measure-Launch $item[0] $item[1] 0
}
for ($round=1; $round -le $Runs; $round++) {
    $order = if ($round % 2) { @(@($BaselineName,$baselinePath), @($CandidateName,$candidatePath)) } else { @(@($CandidateName,$candidatePath), @($BaselineName,$baselinePath)) }
    foreach ($item in $order) {
        $sample = Measure-Launch $item[0] $item[1] $round
        $samples += $sample
        Write-Host "$($sample.app) run $round`: window $($sample.window_ms) ms, $([Math]::Round($sample.private_bytes/1MB,1)) MiB private, $($sample.processes) processes"
    }
}
$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$result = [ordered]@{
    measured_at_utc=[DateTime]::UtcNow.ToString('o'); os=$os.Caption; os_version=$os.Version
    cpu=$cpu.Name; logical_processors=$cpu.NumberOfLogicalProcessors; ram_bytes=[long]$os.TotalVisibleMemorySize*1024
    graphics=@(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion)
    method='One unmeasured warmup per app; interleaved launch order; visible titled EarControl main window, not first painted frame; descendant-tree memory; summed working sets can double-count shared pages. Idle CPU is the delta for processes present at both snapshots; children born and exited between snapshots are not observed.'
    settings=$settingsMethod
    runs=$Runs; settle_seconds=$SettleSeconds; idle_seconds=$IdleSeconds
    baseline=@{ version=(Get-Item $baselinePath).VersionInfo.ProductVersion; bytes=(Get-Item $baselinePath).Length; sha256=(Get-FileHash $baselinePath).Hash.ToLowerInvariant() }
    candidate=@{ version=(Get-Item $candidatePath).VersionInfo.ProductVersion; bytes=(Get-Item $candidatePath).Length; sha256=(Get-FileHash $candidatePath).Hash.ToLowerInvariant() }
    samples=$samples
}
$outputPath = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $outputPath) | Out-Null
$result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
Write-Host "Raw results: $outputPath"
# Keep the isolated benchmark directory available for inspecting a failed run.
