use windows::Win32::Foundation::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::Services::*;
use windows::Win32::System::Threading::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Restore::*;
use windows::core::PCWSTR;
use std::os::windows::process::CommandExt;

/// Prevents a console window from flashing when we spawn powershell/cmd.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ────────────── Registry ──────────────

fn reg_set_dword(subkey: &str, name: &str, value: u32) {
    let wide_key = to_wide(subkey);
    let wide_name = to_wide(name);
    let data = value.to_le_bytes();
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        );
        if result.is_ok() {
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                Some(0),
                REG_DWORD,
                Some(&data),
            );
            let _ = RegCloseKey(hkey);
        }
    }
}

fn reg_set_binary(subkey: &str, name: &str, data: &[u8]) {
    let wide_key = to_wide(subkey);
    let wide_name = to_wide(name);
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        );
        if result.is_ok() {
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                Some(0),
                REG_BINARY,
                Some(data),
            );
            let _ = RegCloseKey(hkey);
        }
    }
}

fn reg_delete_key_tree(subkey: &str) {
    let wide_key = to_wide(subkey);
    unsafe {
        let _ = RegDeleteTreeW(HKEY_LOCAL_MACHINE, PCWSTR(wide_key.as_ptr()));
    }
}

fn reg_get_dword(subkey: &str, name: &str) -> Option<u32> {
    let wide_key = to_wide(subkey);
    let wide_name = to_wide(name);
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        if result.is_ok() {
            let mut value = 0u32;
            let mut size = 4u32;
            let ret = RegQueryValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                None,
                None,
                Some(&mut value as *mut u32 as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(hkey);
            if ret.is_ok() {
                return Some(value);
            }
        }
    }
    None
}

pub fn reg_get_startup_type(name: &str) -> String {
    let key = format!(r"SYSTEM\CurrentControlSet\Services\{}", name);
    match reg_get_dword(&key, "Start") {
        Some(0) => "Boot",
        Some(1) => "System",
        Some(2) => "Auto",
        Some(3) => "Manual",
        Some(4) => "Disabled",
        _ => "Unknown",
    }
    .to_string()
}

// ────────────── Services ──────────────

pub fn stop_service(name: &str) {
    let wide = to_wide(name);
    unsafe {
        if let Ok(mgr) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
            if let Ok(svc) = OpenServiceW(
                mgr,
                PCWSTR(wide.as_ptr()),
                SERVICE_STOP | SERVICE_CHANGE_CONFIG,
            ) {
                let mut status = SERVICE_STATUS::default();
                let _ = ControlService(svc, SERVICE_CONTROL_STOP, &mut status);
                let _ = CloseServiceHandle(svc);
            }
            let _ = CloseServiceHandle(mgr);
        }
    }
}

pub fn set_service_startup(name: &str, start_type: u32) {
    let wide = to_wide(name);
    unsafe {
        if let Ok(mgr) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
            if let Ok(svc) = OpenServiceW(
                mgr,
                PCWSTR(wide.as_ptr()),
                SERVICE_CHANGE_CONFIG,
            ) {
                let _ = ChangeServiceConfigW(
                    svc,
                    ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                    SERVICE_START_TYPE(start_type),
                    SERVICE_ERROR(SERVICE_NO_CHANGE),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                );
                let _ = CloseServiceHandle(svc);
            }
            let _ = CloseServiceHandle(mgr);
        }
    }
}

pub fn start_service(name: &str) {
    let wide = to_wide(name);
    unsafe {
        if let Ok(mgr) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
            if let Ok(svc) = OpenServiceW(mgr, PCWSTR(wide.as_ptr()), SERVICE_START) {
                let _ = StartServiceW(svc, None);
                let _ = CloseServiceHandle(svc);
            }
            let _ = CloseServiceHandle(mgr);
        }
    }
}

pub fn set_driver_startup(name: &str, start: u32) {
    let key = format!(r"SYSTEM\CurrentControlSet\Services\{}", name);
    reg_set_dword(&key, "Start", start);
}

// ────────────── Process ──────────────

/// Case-insensitive exact match of a NUL-terminated PROCESSENTRY32W image name.
pub fn exe_name_matches(sz: &[u16], target: &str) -> bool {
    let end = sz.iter().position(|&c| c == 0).unwrap_or(sz.len());
    String::from_utf16_lossy(&sz[..end]).eq_ignore_ascii_case(target)
}

fn kill_process(name: &str) {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if let Ok(snapshot) = snapshot {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    if exe_name_matches(&entry.szExeFile, name) {
                        if let Ok(proc) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                            let _ = TerminateProcess(proc, 0);
                            let _ = CloseHandle(proc);
                        }
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
}

// ────────────── SmartScreen ──────────────

pub fn kill_smartscreen() {
    kill_process("smartscreen.exe");
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows\System", "EnableSmartScreen", 0);
}

// ────────────── Tamper Protection ──────────────

pub fn disable_tamper_protection() {
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableTamperProtection", 1);
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiSpyware", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Features", "TamperProtection", 0);
}

pub fn enable_tamper_protection() {
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Features", "TamperProtection", 5);
}

// ────────────── WdFilter Altitude (kernel driver blocker) ──────────────

/// Deleting WdFilter's Altitude key prevents the kernel minidriver from loading.
/// This is the most reliable way to disable Defender at kernel level.
/// See: AlteredSecurity/Disable-TamperProtection
pub fn delete_wdfilter_altitude() {
    reg_delete_key_value(r"SYSTEM\CurrentControlSet\Services\WdFilter", "Altitude");
}

pub fn restore_wdfilter_altitude() {
    let wide_key = to_wide(r"SYSTEM\CurrentControlSet\Services\WdFilter");
    let wide_name = to_wide("Altitude");
    let wide_val = to_wide("328010");
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        );
        if result.is_ok() {
            let _ = RegSetValueExW(
                hkey,
                PCWSTR(wide_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(wide_val.as_slice().as_bytes()),
            );
            let _ = RegCloseKey(hkey);
        }
    }
}

fn reg_delete_key_value(subkey: &str, name: &str) {
    let wide_key = to_wide(subkey);
    let wide_name = to_wide(name);
    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            KEY_ALL_ACCESS,
            &mut hkey,
        );
        if result.is_ok() {
            let _ = RegDeleteValueW(hkey, PCWSTR(wide_name.as_ptr()));
            let _ = RegCloseKey(hkey);
        }
    }
}

// ────────────── Registry Sets ──────────────

pub fn set_registry_disable() {
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiSpyware", 1);
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiVirus", 1);
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableTamperProtection", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender", "DisableAntiSpyware", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender", "DisableAntiVirus", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableRealtimeMonitoring", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableBehaviorMonitoring", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableOnAccessProtection", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableScanOnRealtimeEnable", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\SpyNet", "DisableBlockAtFirstSeen", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\SpyNet", "SubmitSamplesConsent", 2);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\SpyNet", "MAPSReporting", 0);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WinDefend", "Start", 4);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdFilter", "Start", 4);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdNisDrv", "Start", 4);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdNisSvc", "Start", 4);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdBoot", "Start", 4);
    reg_set_binary(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        "SecurityHealth",
        &[3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
}

pub fn set_registry_enable() {
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiSpyware", 0);
    reg_set_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiVirus", 0);
    reg_delete_key_value(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableTamperProtection");
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender", "DisableAntiSpyware", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender", "DisableAntiVirus", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableRealtimeMonitoring", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableBehaviorMonitoring", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableOnAccessProtection", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableScanOnRealtimeEnable", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\SpyNet", "DisableBlockAtFirstSeen", 0);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\SpyNet", "SubmitSamplesConsent", 1);
    reg_set_dword(r"SOFTWARE\Microsoft\Windows Defender\SpyNet", "MAPSReporting", 2);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WinDefend", "Start", 2);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdFilter", "Start", 0);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdNisDrv", "Start", 3);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdNisSvc", "Start", 3);
    reg_set_dword(r"SYSTEM\CurrentControlSet\Services\WdBoot", "Start", 0);
    reg_set_binary(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run",
        "SecurityHealth",
        &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    );
}

// ────────────── MpPreference (via PowerShell) ──────────────

pub fn set_mp_preference_disable() {
    let script = r#"
$p = @{
    DisableRealtimeMonitoring = $true
    DisableBehaviorMonitoring = $true
    DisableBlockAtFirstSeen = $true
    DisableIOAVProtection = $true
    DisablePrivacyMode = $true
    DisableArchiveScanning = $true
    DisableIntrusionPreventionSystem = $true
    DisableScriptScanning = $true
    DisableAntiSpyware = $true
    DisableAntiVirus = $true
    EnableControlledFolderAccess = "Disabled"
    PUAProtection = "Disabled"
    SubmitSamplesConsent = 2
    MAPSReporting = 0
    SignatureDisableUpdateOnStartupWithoutEngine = $true
}
Set-MpPreference @p
"#;
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

pub fn set_mp_preference_enable() {
    let script = r#"
$p = @{
    DisableRealtimeMonitoring = $false
    DisableBehaviorMonitoring = $false
    DisableBlockAtFirstSeen = $false
    DisableIOAVProtection = $false
    DisablePrivacyMode = $false
    DisableArchiveScanning = $false
    DisableIntrusionPreventionSystem = $false
    DisableScriptScanning = $false
    DisableAntiSpyware = $false
    DisableAntiVirus = $false
    EnableControlledFolderAccess = "Enabled"
    PUAProtection = "Enabled"
    SubmitSamplesConsent = 1
    MAPSReporting = 2
    SignatureDisableUpdateOnStartupWithoutEngine = $false
}
Set-MpPreference @p
"#;
    let _ = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

// ────────────── IFEO ──────────────

pub fn set_ifeo_debugger() {
    let targets = [
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\mpcmdrun.exe",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\mpcmdrun.exe",
    ];
    for target in &targets {
        let wide_key = to_wide(target);
        let wide_name = to_wide("Debugger");
        let wide_val = to_wide("rundll32.exe");
        unsafe {
            let mut hkey = HKEY::default();
            let result = RegCreateKeyExW(
                HKEY_LOCAL_MACHINE,
                PCWSTR(wide_key.as_ptr()),
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_ALL_ACCESS,
                None,
                &mut hkey,
                None,
            );
            if result.is_ok() {
                let _ = RegSetValueExW(
                    hkey,
                    PCWSTR(wide_name.as_ptr()),
                    Some(0),
                    REG_SZ,
                    Some(wide_val.as_slice().as_bytes()),
                );
                let _ = RegCloseKey(hkey);
            }
        }
    }
}

pub fn remove_ifeo_debugger() {
    let targets = [
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\mpcmdrun.exe",
        r"SOFTWARE\WOW6432Node\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\mpcmdrun.exe",
    ];
    for target in &targets {
        reg_delete_key_tree(target);
    }
}

// ────────────── Service Status ──────────────

pub struct ServiceInfo {
    pub name: String,
    pub status: String,
    pub startup_type: String,
}

fn get_service_state(name: &str) -> String {
    let wide = to_wide(name);
    unsafe {
        if let Ok(mgr) = OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
            if let Ok(svc) = OpenServiceW(
                mgr,
                PCWSTR(wide.as_ptr()),
                SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG,
            ) {
                let mut status = SERVICE_STATUS_PROCESS::default();
                let mut needed = 0u32;
                let buf_slice = std::slice::from_raw_parts_mut(
                    &mut status as *mut _ as *mut u8,
                    std::mem::size_of::<SERVICE_STATUS_PROCESS>(),
                );
                let ret = QueryServiceStatusEx(
                    svc,
                    SC_STATUS_PROCESS_INFO,
                    Some(buf_slice),
                    &mut needed,
                );
                let state = if ret.is_ok() {
                    match status.dwCurrentState.0 {
                        1 => "Stopped",
                        2 => "Start Pending",
                        3 => "Stop Pending",
                        4 => "Running",
                        5 => "Continue Pending",
                        6 => "Pause Pending",
                        7 => "Paused",
                        _ => "Unknown",
                    }
                    .to_string()
                } else {
                    "N/A".to_string()
                };
                let _ = CloseServiceHandle(svc);
                let _ = CloseServiceHandle(mgr);
                return state;
            }
            let _ = CloseServiceHandle(mgr);
        }
    }
    "N/A".to_string()
}

pub fn get_service_status() -> Vec<ServiceInfo> {
    let names = ["WinDefend", "WdNisSvc", "SecurityHealthService", "wscsvc", "WdFilter", "WdNisDrv"];
    names
        .iter()
        .map(|name| ServiceInfo {
            name: name.to_string(),
            status: get_service_state(name),
            startup_type: reg_get_startup_type(name),
        })
        .collect()
}

// ────────────── WMI Status ──────────────

#[derive(Default)]
pub struct DefenderStatus {
    pub overall_status: String,
    pub real_time_protection_on: bool,
    pub antivirus_enabled: bool,
    pub tamper_protection_on: bool,
    pub services: Vec<ServiceInfo>,
}

fn reg_check_dword(subkey: &str, name: &str) -> Option<u32> {
    reg_get_dword(subkey, name)
}

pub fn get_status() -> DefenderStatus {
    let services = get_service_status();

    // Check registry first (definitive, works even without PowerShell)
    let disable_policy = reg_check_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiSpyware");
    let disable_local = reg_check_dword(r"SOFTWARE\Microsoft\Windows Defender", "DisableAntiSpyware");
    let disable_rtm = reg_check_dword(r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection", "DisableRealtimeMonitoring");
    let tamper_policy = reg_check_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableTamperProtection");

    // PowerShell as secondary source
    let script = r#"
$status = Get-MpComputerStatus -ErrorAction SilentlyContinue
if ($status) {
    Write-Output "RTP=$($status.RealTimeProtectionEnabled)"
    Write-Output "AV=$($status.AntivirusEnabled)"
}
$prefs = Get-MpPreference -ErrorAction SilentlyContinue
if ($prefs) {
    Write-Output "DRM=$($prefs.DisableRealtimeMonitoring)"
}
"#;

    let mut ps_realtime = true;
    let mut ps_antivirus = true;
    if let Ok(out) = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if let Some(val) = line.strip_prefix("RTP=") {
                ps_realtime = val.trim() == "True";
            }
            if let Some(val) = line.strip_prefix("AV=") {
                ps_antivirus = val.trim() == "True";
            }
            if let Some(val) = line.strip_prefix("DRM=") {
                if val.trim() == "True" {
                    ps_realtime = false;
                }
            }
        }
    }

    // Registry overrides PowerShell: if DisableAntiSpyware=1 or DisableRealtimeMonitoring=1, consider it disabled
    let realtime = if disable_rtm == Some(1) || disable_local == Some(1) || disable_policy == Some(1) {
        false
    } else {
        ps_realtime
    };
    let antivirus = if disable_local == Some(1) || disable_policy == Some(1) {
        false
    } else {
        ps_antivirus
    };

    let tp = reg_get_dword(r"SOFTWARE\Microsoft\Windows Defender\Features", "TamperProtection");
    let tamper_from_features = tp == Some(5);
    let tamper = tamper_policy != Some(1) && tamper_from_features;

    let overall = if !realtime && !antivirus {
        "Disabled"
    } else if realtime {
        "Protected"
    } else {
        "Partial"
    }
    .to_string();

    DefenderStatus {
        overall_status: overall,
        real_time_protection_on: realtime,
        antivirus_enabled: antivirus,
        tamper_protection_on: tamper,
        services,
    }
}

// ────────────── Utility for u16 → &[u8] ──────────────

trait AsBytes {
    fn as_bytes(&self) -> &[u8];
}

impl AsBytes for [u16] {
    fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.as_ptr() as *const u8, self.len() * 2) }
    }
}

// ────────────── Orchestration ──────────────

#[allow(dead_code)]
pub fn disable_all() {
    disable_tamper_protection();
    kill_smartscreen();
    for svc in &["WinDefend", "WdNisSvc", "SecurityHealthService", "wscsvc"] {
        stop_service(svc);
        set_service_startup(svc, 4);
    }
    for drv in &["WdFilter", "WdNisDrv", "WdBoot"] {
        set_driver_startup(drv, 4);
    }
    set_registry_disable();
    delete_wdfilter_altitude();
    set_mp_preference_disable();
    set_ifeo_debugger();
}

#[allow(dead_code)]
pub fn enable_all() {
    enable_tamper_protection();
    restore_wdfilter_altitude();
    for (name, start) in &[("wscsvc", 2u32), ("SecurityHealthService", 2u32), ("WinDefend", 2u32), ("WdNisSvc", 3u32)] {
        set_service_startup(name, *start);
        start_service(name);
    }
    for (name, start) in &[("WdFilter", 0u32), ("WdBoot", 0u32), ("WdNisDrv", 3u32)] {
        set_driver_startup(name, *start);
    }
    set_registry_enable();
    set_mp_preference_enable();
    remove_ifeo_debugger();
}

// ────────────── PPL Stripping ──────────────

pub fn strip_service_ppl(name: &str) {
    let key = format!(r"SYSTEM\CurrentControlSet\Services\{}", name);
    reg_set_dword(&key, "LaunchProtected", 0);
}

pub fn restore_service_ppl(name: &str) {
    let key = format!(r"SYSTEM\CurrentControlSet\Services\{}", name);
    reg_delete_key_value(&key, "LaunchProtected");
}

// ────────────── System Restore Point ──────────────

fn str_to_u16_array(s: &str) -> [u16; 256] {
    let mut arr = [0u16; 256];
    let wide: Vec<u16> = s.encode_utf16().take(255).collect();
    arr[..wide.len()].copy_from_slice(&wide);
    arr
}

pub fn create_restore_point() {
    unsafe {
        let mut status = STATEMGRSTATUS::default();
        let desc_arr = str_to_u16_array("Defender Control - 系统配置更改");

        let rp = RESTOREPOINTINFOW {
            dwEventType: BEGIN_SYSTEM_CHANGE,
            dwRestorePtType: MODIFY_SETTINGS,
            llSequenceNumber: 0,
            szDescription: desc_arr,
        };

        let _ = SRSetRestorePointW(&rp, &mut status);
        std::thread::sleep(std::time::Duration::from_millis(500));

        let rp2 = RESTOREPOINTINFOW {
            dwEventType: END_SYSTEM_CHANGE,
            dwRestorePtType: MODIFY_SETTINGS,
            llSequenceNumber: status.llSequenceNumber,
            szDescription: [0u16; 256],
        };
        let _ = SRSetRestorePointW(&rp2, &mut status);
    }
}

// ────────────── Registry Backup / Restore ──────────────

fn backup_dir() -> std::path::PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(base).join("DefenderControl").join("backup")
}

/// (filename, registry key) pairs we snapshot before disabling.
const BACKUP_KEYS: &[(&str, &str)] = &[
    ("def_local", r"HKLM\SOFTWARE\Microsoft\Windows Defender"),
    ("def_policy", r"HKLM\SOFTWARE\Policies\Microsoft\Windows Defender"),
    ("windefend", r"HKLM\SYSTEM\CurrentControlSet\Services\WinDefend"),
    ("wdfilter", r"HKLM\SYSTEM\CurrentControlSet\Services\WdFilter"),
    ("wdnisdrv", r"HKLM\SYSTEM\CurrentControlSet\Services\WdNisDrv"),
    ("wdnissvc", r"HKLM\SYSTEM\CurrentControlSet\Services\WdNisSvc"),
    ("wdboot", r"HKLM\SYSTEM\CurrentControlSet\Services\WdBoot"),
    ("sechealth", r"HKLM\SYSTEM\CurrentControlSet\Services\SecurityHealthService"),
];

/// Export current Defender-related registry to .reg files (best effort).
pub fn backup_registry() {
    let dir = backup_dir();
    let _ = std::fs::create_dir_all(&dir);
    for (file, key) in BACKUP_KEYS {
        let path = dir.join(format!("{}.reg", file));
        let arg = format!("reg export \"{}\" \"{}\" /y", key, path.display());
        let _ = std::process::Command::new("cmd")
            .args(["/C", &arg])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

/// True if a previous backup exists on disk.
#[allow(dead_code)]
pub fn has_backup() -> bool {
    let dir = backup_dir();
    BACKUP_KEYS
        .iter()
        .any(|(f, _)| dir.join(format!("{}.reg", f)).exists())
}

/// Restore Defender registry from backup. Returns true if at least one file imported.
pub fn restore_registry_from_backup() -> bool {
    let dir = backup_dir();
    let mut any = false;
    for (file, _key) in BACKUP_KEYS {
        let path = dir.join(format!("{}.reg", file));
        if path.exists() {
            let arg = format!("reg import \"{}\"", path.display());
            let ok = std::process::Command::new("cmd")
                .args(["/C", &arg])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            any = any || ok;
        }
    }
    any
}

// ────────────── Scheduled Watch Task ──────────────

pub fn install_watch_task() {
    let exe = std::env::current_exe().ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "defender-control.exe".to_string());

    let script = format!(
        r#"schtasks /Create /SC ONLOGON /TN "DefenderControlWatch" /TR "\"{}\" --watch" /RL HIGHEST /F"#,
        exe
    );

    let _ = std::process::Command::new("cmd")
        .args(["/C", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

pub fn remove_watch_task() {
    let _ = std::process::Command::new("cmd")
        .args(["/C", r#"schtasks /Delete /TN "DefenderControlWatch" /F"#])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

pub fn is_watch_installed() -> bool {
    let out = std::process::Command::new("cmd")
        .args(["/C", r#"schtasks /Query /TN "DefenderControlWatch" /FO CSV"#])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        });
    out.is_some_and(|s| s.contains("DefenderControlWatch"))
}

// ────────────── Watch Mode (headless, continuous) ──────────────

/// Registry-only quick check (no PowerShell) used by the watcher hot loop.
/// Returns true if Defender appears to be (re-)enabled.
fn defender_looks_enabled() -> bool {
    let dis_policy = reg_get_dword(r"SOFTWARE\Policies\Microsoft\Windows Defender", "DisableAntiSpyware");
    let dis_local = reg_get_dword(r"SOFTWARE\Microsoft\Windows Defender", "DisableAntiSpyware");
    let dis_rtm = reg_get_dword(
        r"SOFTWARE\Microsoft\Windows Defender\Real-Time Protection",
        "DisableRealtimeMonitoring",
    );
    let start = reg_get_dword(r"SYSTEM\CurrentControlSet\Services\WinDefend", "Start");
    let disabled = start == Some(4)
        && (dis_local == Some(1) || dis_policy == Some(1) || dis_rtm == Some(1));
    !disabled
}

/// Lean re-suppress (no PowerShell) — keeps the watcher loop light.
fn apply_disable_lean() {
    if let Ok(_guard) = crate::trusted_installer::escalate_to_ti() {
        disable_tamper_protection();
        for svc in &["WinDefend", "WdNisSvc", "SecurityHealthService", "wscsvc"] {
            stop_service(svc);
            set_service_startup(svc, 4);
        }
        for drv in &["WdFilter", "WdNisDrv", "WdBoot"] {
            set_driver_startup(drv, 4);
        }
        set_registry_disable();
        delete_wdfilter_altitude();
        set_ifeo_debugger();
    }
}

/// Block until the Defender configuration key changes (event-driven, ~0 CPU).
/// Falls back to a 10s sleep if the key can't be opened for notification.
fn wait_for_defender_change() {
    let key = to_wide(r"SOFTWARE\Microsoft\Windows Defender");
    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            Some(0),
            KEY_NOTIFY,
            &mut hkey,
        )
        .is_ok()
        {
            let _ = RegNotifyChangeKeyValue(
                hkey,
                true,
                REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                None,
                false,
            );
            let _ = RegCloseKey(hkey);
        } else {
            std::thread::sleep(std::time::Duration::from_secs(10));
        }
    }
}

pub fn run_watch_mode() {
    // Re-apply once at startup, then watch the Defender key forever and
    // re-suppress whenever Windows or the user tries to turn it back on.
    apply_disable_lean();
    loop {
        wait_for_defender_change();
        if defender_looks_enabled() {
            apply_disable_lean();
        }
    }
}
