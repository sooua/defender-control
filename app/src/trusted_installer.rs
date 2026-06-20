use std::ffi::c_void;
use std::mem;
use windows::Win32::Foundation::*;
use windows::Win32::Security::*;
use windows::Win32::System::Services::*;
use windows::Win32::System::Threading::*;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::core::PCWSTR;

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// RAII guard that reverts impersonation on drop.
pub struct TiGuard;

impl Drop for TiGuard {
    fn drop(&mut self) {
        let _ = unsafe { RevertToSelf() };
    }
}

/// Check if current process is running as administrator.
pub fn is_admin() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut size = mem::size_of::<TOKEN_ELEVATION>() as u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            size,
            &mut size,
        );
        let _ = CloseHandle(token);
        ok.is_ok() && elevation.TokenIsElevated != 0
    }
}

fn enable_privilege(name: &str) -> Result<(), String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_ADJUST_PRIVILEGES,
            &mut token,
        )
        .map_err(|e| format!("OpenProcessToken failed: {}", e))?;

        let wide = to_wide(name);
        let mut luid = LUID::default();
        LookupPrivilegeValueW(None, PCWSTR(wide.as_ptr()), &mut luid)
            .map_err(|e| format!("LookupPrivilegeValueW({}) failed: {}", name, e))?;

        let mut tp = TOKEN_PRIVILEGES::default();
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Luid = luid;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

        AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None)
            .map_err(|e| format!("AdjustTokenPrivileges({}) failed: {}", name, e))?;

        let _ = CloseHandle(token);
        Ok(())
    }
}

fn find_process_pid(name: &str) -> Result<u32, String> {
    unsafe {
        let wide_name = to_wide(name);
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| format!("CreateToolhelp32Snapshot failed: {}", e))?;

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut pid = None;
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.szExeFile.starts_with(&wide_name) {
                    pid = Some(entry.th32ProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        pid.ok_or_else(|| format!("Process '{}' not found", name))
    }
}

fn impersonate_system() -> Result<(), String> {
    enable_privilege("SeDebugPrivilege")?;
    enable_privilege("SeImpersonatePrivilege")?;

    let winlogon_pid = find_process_pid("winlogon.exe")?;

    unsafe {
        let proc = OpenProcess(
            PROCESS_DUP_HANDLE | PROCESS_QUERY_INFORMATION,
            false,
            winlogon_pid,
        )
        .map_err(|e| format!("OpenProcess(winlogon.exe) failed: {}", e))?;

        let mut token = HANDLE::default();
        OpenProcessToken(proc, TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY | TOKEN_IMPERSONATE, &mut token)
            .map_err(|e| format!("OpenProcessToken(winlogon.exe) failed: {}", e))?;
        let _ = CloseHandle(proc);

        let mut dup = HANDLE::default();
        DuplicateTokenEx(
            token,
            TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY | TOKEN_IMPERSONATE,
            None,
            SecurityImpersonation,
            TokenImpersonation,
            &mut dup,
        )
        .map_err(|e| format!("DuplicateTokenEx(winlogon.exe) failed: {}", e))?;
        let _ = CloseHandle(token);

        ImpersonateLoggedOnUser(dup)
            .map_err(|e| format!("ImpersonateLoggedOnUser failed: {}", e))?;
        let _ = CloseHandle(dup);
    }

    Ok(())
}

fn start_ti_service() -> Result<u32, String> {
    unsafe {
        let mgr = OpenSCManagerW(None, None, SC_MANAGER_CONNECT)
            .map_err(|e| format!("OpenSCManagerW failed: {}", e))?;

        let wide = to_wide("TrustedInstaller");
        let svc = OpenServiceW(
            mgr,
            PCWSTR(wide.as_ptr()),
            SERVICE_QUERY_STATUS | SERVICE_START | SERVICE_INTERROGATE,
        )
        .map_err(|e| {
            let _ = CloseServiceHandle(mgr);
            format!("OpenServiceW(TrustedInstaller) failed: {}", e)
        })?;

        let mut status = SERVICE_STATUS_PROCESS::default();
        let mut needed = 0u32;

        let pid = loop {
            let buf = std::slice::from_raw_parts_mut(
                &mut status as *mut _ as *mut u8,
                mem::size_of::<SERVICE_STATUS_PROCESS>(),
            );
            let qr = QueryServiceStatusEx(
                svc,
                SC_STATUS_PROCESS_INFO,
                Some(buf),
                &mut needed,
            );

            if qr.is_err() {
                let _ = CloseServiceHandle(svc);
                let _ = CloseServiceHandle(mgr);
                return Err("QueryServiceStatusEx failed".into());
            }

            match status.dwCurrentState {
                s if s == SERVICE_STOPPED => {
                    if StartServiceW(svc, None).is_err() {
                        let _ = CloseServiceHandle(svc);
                        let _ = CloseServiceHandle(mgr);
                        return Err("StartServiceW(TrustedInstaller) failed".into());
                    }
                }
                s if s == SERVICE_START_PENDING || s == SERVICE_STOP_PENDING => {
                    let wait = if status.dwWaitHint > 100 {
                        status.dwWaitHint
                    } else {
                        2000
                    };
                    std::thread::sleep(std::time::Duration::from_millis(wait as u64));
                    continue;
                }
                s if s == SERVICE_RUNNING => {
                    break status.dwProcessId;
                }
                _ => {
                    let _ = CloseServiceHandle(svc);
                    let _ = CloseServiceHandle(mgr);
                    return Err(format!(
                        "TrustedInstaller unexpected state: {}",
                        status.dwCurrentState.0
                    ));
                }
            }
        };

        let _ = CloseServiceHandle(svc);
        let _ = CloseServiceHandle(mgr);
        Ok(pid)
    }
}

fn impersonate_ti_process(pid: u32) -> Result<(), String> {
    unsafe {
        let proc = OpenProcess(
            PROCESS_DUP_HANDLE | PROCESS_QUERY_INFORMATION,
            false,
            pid,
        )
        .map_err(|e| format!("OpenProcess(TrustedInstaller.exe) failed: {}", e))?;

        let mut token = HANDLE::default();
        OpenProcessToken(proc, TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY | TOKEN_IMPERSONATE, &mut token)
            .map_err(|e| format!("OpenProcessToken(TrustedInstaller.exe) failed: {}", e))?;
        let _ = CloseHandle(proc);

        let mut dup = HANDLE::default();
        DuplicateTokenEx(
            token,
            TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY | TOKEN_IMPERSONATE,
            None,
            SecurityImpersonation,
            TokenImpersonation,
            &mut dup,
        )
        .map_err(|e| format!("DuplicateTokenEx(TrustedInstaller) failed: {}", e))?;
        let _ = CloseHandle(token);

        ImpersonateLoggedOnUser(dup)
            .map_err(|e| format!("ImpersonateLoggedOnUser(TrustedInstaller) failed: {}", e))?;
        let _ = CloseHandle(dup);
    }

    Ok(())
}

/// Escalate current thread to TrustedInstaller level.
/// Returns a guard that automatically reverts on drop.
pub fn escalate_to_ti() -> Result<TiGuard, String> {
    impersonate_system()?;
    let pid = start_ti_service()?;
    impersonate_ti_process(pid)?;
    Ok(TiGuard)
}
