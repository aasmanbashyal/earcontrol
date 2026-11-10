param(
    [switch]$RendererOnly,
    [string]$ProbeExe,
    [string[]]$ProbeArguments = @(),
    [string]$ProbeName = 'isolated_renderer',
    [string]$OutputName = '.tools\memory-diagnostics-custom.json'
)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text;

public static class OwnedMemoryProbe {
    [StructLayout(LayoutKind.Sequential)]
    struct Region {
        public ulong BaseAddress, AllocationBase;
        public uint AllocationProtect, Alignment1;
        public ulong RegionSize;
        public uint State, Protect, Type, Alignment2;
    }
    [StructLayout(LayoutKind.Sequential)]
    struct WorkingPage { public UIntPtr Address, Attributes; }
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern IntPtr OpenProcess(uint access, bool inherit, int id);
    [DllImport("kernel32.dll")]
    static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern UIntPtr VirtualQueryEx(IntPtr process, UIntPtr address, out Region region, UIntPtr length);
    [DllImport("psapi.dll", SetLastError=true)]
    static extern bool QueryWorkingSetEx(IntPtr process, [In, Out] WorkingPage[] pages, int size);
    [DllImport("psapi.dll", CharSet=CharSet.Unicode)]
    static extern uint GetMappedFileNameW(IntPtr process, UIntPtr address, StringBuilder name, uint size);

    public class Bucket {
        public string Kind, Protection, File;
        public ulong CommittedBytes, ResidentBytes, ResidentPrivateBytes, ResidentSharedBytes;
        public int Regions;
    }
    public class Module {
        public string Name, Path;
        public long ImageVirtualBytes;
    }
    public class Snapshot {
        public int ProcessId;
        public long PrivateBytes, WorkingSetBytes, PeakWorkingSetBytes;
        public List<Bucket> ByTypeProtection = new List<Bucket>();
        public List<Bucket> Allocations = new List<Bucket>();
        public List<Module> Modules = new List<Module>();
    }
    static string Kind(uint type) {
        return type == 0x20000 ? "private" : type == 0x40000 ? "mapped" : type == 0x1000000 ? "image" : "other";
    }
    static string Protection(uint protect) {
        string name;
        switch(protect & 255) {
            case 1: name="no_access"; break;
            case 2: name="read_only"; break;
            case 4: name="read_write"; break;
            case 8: name="write_copy"; break;
            case 16: name="execute"; break;
            case 32: name="execute_read"; break;
            case 64: name="execute_read_write"; break;
            case 128: name="execute_write_copy"; break;
            default: name="unknown"; break;
        }
        return name + ((protect & 256) != 0 ? "+guard" : "");
    }
    static void Add(Bucket b, Region r, ulong resident, ulong privateResident) {
        b.CommittedBytes += r.RegionSize;
        b.ResidentBytes += resident;
        b.ResidentPrivateBytes += privateResident;
        b.ResidentSharedBytes += resident - privateResident;
        b.Regions++;
    }
    public static Snapshot Read(int id) {
        // Only virtual-region, page-residency and module metadata are queried.
        // No ReadProcessMemory or other process-memory-content API is used.
        IntPtr handle = OpenProcess(0x410, false, id);
        if(handle == IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
        try {
            var process=Process.GetProcessById(id);
            process.Refresh();
            var result = new Snapshot { ProcessId=id, PrivateBytes=process.PrivateMemorySize64,
                WorkingSetBytes=process.WorkingSet64, PeakWorkingSetBytes=process.PeakWorkingSet64 };
            var groups=new Dictionary<string, Bucket>();
            var allocations=new Dictionary<ulong, Bucket>();
            ulong address=0;
            Region r;
            while(VirtualQueryEx(handle, (UIntPtr)address, out r, (UIntPtr)Marshal.SizeOf(typeof(Region))) != UIntPtr.Zero) {
                if(r.RegionSize == 0 || r.BaseAddress + r.RegionSize <= address) break;
                address=r.BaseAddress+r.RegionSize;
                if(r.State != 0x1000) continue;
                ulong resident=0, privateResident=0;
                for(ulong offset=0; offset<r.RegionSize;) {
                    int count=(int)Math.Min(8192UL, (r.RegionSize-offset+4095)/4096);
                    var pages=new WorkingPage[count];
                    for(int i=0; i<count; i++) pages[i].Address=(UIntPtr)(r.BaseAddress+offset+(ulong)i*4096);
                    if(!QueryWorkingSetEx(handle,pages,count*Marshal.SizeOf(typeof(WorkingPage))))
                        throw new System.ComponentModel.Win32Exception();
                    foreach(var page in pages) {
                        ulong flags=page.Attributes.ToUInt64();
                        if((flags & 1)!=0) {
                            resident+=4096;
                            if((flags & (1UL<<15))==0) privateResident+=4096;
                        }
                    }
                    offset+=(ulong)count*4096;
                }
                string kind=Kind(r.Type), protection=Protection(r.Protect), key=kind+":"+protection;
                Bucket group;
                if(!groups.TryGetValue(key,out group)) groups[key]=group=new Bucket { Kind=kind, Protection=protection };
                Add(group,r,resident,privateResident);
                Bucket allocation;
                if(!allocations.TryGetValue(r.AllocationBase,out allocation)) {
                    string file=null;
                    if(r.Type==0x40000 || r.Type==0x1000000) {
                        var buffer=new StringBuilder(2048);
                        if(GetMappedFileNameW(handle,(UIntPtr)r.BaseAddress,buffer,2048)>0) file=buffer.ToString();
                    }
                    allocations[r.AllocationBase]=allocation=new Bucket { Kind=kind, File=file };
                }
                Add(allocation,r,resident,privateResident);
            }
            result.ByTypeProtection.AddRange(groups.Values);
            result.Allocations.AddRange(allocations.Values);
            foreach(ProcessModule module in process.Modules) result.Modules.Add(new Module {
                Name=module.ModuleName, Path=module.FileName, ImageVirtualBytes=module.ModuleMemorySize });
            return result;
        } finally { CloseHandle(handle); }
    }
}
'@

$rootPath = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$scratch = Join-Path $rootPath ('.tools\memory-probe-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$candidateDir = Join-Path $scratch 'candidate'
New-Item -ItemType Directory -Path $candidateDir | Out-Null
$candidate = Join-Path $candidateDir 'EarControl.exe'
if(!$ProbeExe -and !$RendererOnly) {
    Copy-Item -LiteralPath (Join-Path $rootPath '.tools\size-profile-z.exe') -Destination $candidate
}
$settingsPath = Join-Path $scratch 'profiles.json'
$originalSettings = Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'app.earcontrol.desktop\profiles.json'
$originalHash = $null
if(Test-Path -LiteralPath $originalSettings) {
    $originalHash = (Get-FileHash -LiteralPath $originalSettings).Hash
    Copy-Item -LiteralPath $originalSettings -Destination $settingsPath
}
$samples = @()
$entries = @(
    @{Name='rust_3_0_baseline';Path=(Join-Path $rootPath '.tools\rust-port-optimization-baseline\EarControl.exe')},
    @{Name='optimized_preview_z';Path=$candidate}
)
if($RendererOnly) {
    $rendererPath = Join-Path $rootPath 'target\release\examples\renderer_baseline.exe'
    $entries = @(
        @{Name='empty_eframe_glow';Path=$rendererPath},
        @{Name='empty_eframe_glow_native_init';Path=$rendererPath;Arguments=@('--native-init')}
    )
}
if($ProbeExe) {
    $entries = @(@{Name=$ProbeName;Path=(Resolve-Path -LiteralPath $ProbeExe).Path;Arguments=$ProbeArguments})
}
foreach($entry in $entries) {
    $start = [Diagnostics.ProcessStartInfo]::new($entry.Path)
    $start.UseShellExecute=$false
    $start.WindowStyle=[Diagnostics.ProcessWindowStyle]::Hidden
    $start.WorkingDirectory=Split-Path -Parent $entry.Path
    $start.Environment['EARCONTROL_SETTINGS_PATH']=$settingsPath
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    foreach($argument in $entry.Arguments) { [void]$start.ArgumentList.Add($argument) }
    $process=[Diagnostics.Process]::Start($start)
    $stdout=$process.StandardOutput.ReadToEndAsync()
    $stderr=$process.StandardError.ReadToEndAsync()
    try {
        Start-Sleep -Seconds 8
        if($process.HasExited) { throw "$($entry.Name) exited unexpectedly" }
        $snapshot=[OwnedMemoryProbe]::Read($process.Id)
        $sample = [pscustomobject]@{name=$entry.Name;sha256=(Get-FileHash -LiteralPath $entry.Path).Hash.ToLowerInvariant();settle_seconds=8;snapshot=$snapshot;stdout='';stderr=''}
        $samples += $sample
        Write-Host "$($entry.Name): private $([Math]::Round($snapshot.PrivateBytes/1MB,2)) MiB, working set $([Math]::Round($snapshot.WorkingSetBytes/1MB,2)) MiB"
    } finally {
        if(!$process.HasExited) {
            [void]$process.CloseMainWindow()
            if(!$process.WaitForExit(5000)) { $process.Kill(); $process.WaitForExit() }
        }
        if($sample) { $sample.stdout=$stdout.GetAwaiter().GetResult(); $sample.stderr=$stderr.GetAwaiter().GetResult() }
        $process.Dispose()
    }
}
if($originalHash -and (Get-FileHash -LiteralPath $originalSettings).Hash -ne $originalHash) { throw 'Original settings changed unexpectedly' }
$output=Join-Path $rootPath $(if($ProbeExe){$OutputName}elseif($RendererOnly){'.tools\memory-diagnostics-renderer.json'}else{'.tools\memory-diagnostics-native.json'})
[pscustomobject]@{measured_at_utc=[DateTime]::UtcNow.ToString('o');method='One sequential diagnostic sample per executable after eight seconds, isolated shared settings copy. VirtualQueryEx committed regions; QueryWorkingSetEx page metadata; loaded module metadata. No process memory content read. No CPU/startup comparison.';samples=$samples} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $output -Encoding utf8
Write-Host "Saved $output"
