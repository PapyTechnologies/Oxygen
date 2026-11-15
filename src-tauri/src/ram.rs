//! Module de surveillance de la mémoire vive (RAM) de LightBrowser.
//!
//! Mesure basée sur la mémoire privée (PrivateUsage via GetProcessMemoryInfo),
//! identique à la colonne Mémoire privée du Gestionnaire des tâches Windows.
//! L'arbre complet des descendants (LightBrowser.exe + msedgewebview2.exe) est parcouru
//! via CreateToolhelp32Snapshot sans doublon et sans compter les WebView2 d'autres applications.

use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{Emitter, Manager};

#[derive(Debug, Clone, Serialize, Default)]
pub struct RamUsage {
    pub total_mb: f64,
    pub app_mb: f64,
    pub webview_mb: f64,
    pub process_count: usize,
    pub webview_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessDiagInfo {
    pub pid: u32,
    pub name: String,
    pub parent_pid: Option<u32>,
    pub private_mb: f64,
    pub working_set_mb: f64,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct ProcessEntry {
    pid: u32,
    parent_pid: u32,
    name: String,
}

#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case)]
struct PROCESS_MEMORY_COUNTERS_EX {
    cb: u32,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
    PrivateUsage: usize,
}

#[cfg(windows)]
#[repr(C)]
#[allow(non_snake_case)]
struct PROCESSENTRY32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: usize,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; 260],
}

#[cfg(windows)]
#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(
        process: isize,
        ppsmc: *mut PROCESS_MEMORY_COUNTERS_EX,
        cb: u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> isize;
    fn Process32FirstW(hSnapshot: isize, lppe: *mut PROCESSENTRY32W) -> i32;
    fn Process32NextW(hSnapshot: isize, lppe: *mut PROCESSENTRY32W) -> i32;
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> isize;
    fn CloseHandle(hObject: isize) -> i32;
}

#[cfg(windows)]
const TH32CS_SNAPPROCESS: u32 = 0x00000002;
#[cfg(windows)]
const INVALID_HANDLE_VALUE: isize = -1;
#[cfg(windows)]
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
#[cfg(windows)]
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;

#[cfg(windows)]
fn snapshot_processes() -> Vec<ProcessEntry> {
    let mut procs = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE || snapshot == 0 {
            return procs;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                procs.push(ProcessEntry {
                    pid: entry.th32ProcessID,
                    parent_pid: entry.th32ParentProcessID,
                    name,
                });
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    procs
}

#[cfg(windows)]
fn get_process_memory(pid: u32) -> Option<(u64, u64)> {
    unsafe {
        let mut handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
        }
        if handle == 0 {
            return None;
        }
        let mut pmc: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let ok = GetProcessMemoryInfo(handle, &mut pmc, pmc.cb);
        CloseHandle(handle);
        if ok != 0 {
            Some((pmc.PrivateUsage as u64, pmc.WorkingSetSize as u64))
        } else {
            None
        }
    }
}

pub struct RamMonitor {
    app_pid: u32,
}

impl RamMonitor {
    pub fn new() -> Self {
        Self {
            app_pid: std::process::id(),
        }
    }

    pub fn collect(&mut self) -> RamUsage {
        #[cfg(windows)]
        {
            let procs = snapshot_processes();
            let mut tree_pids = HashSet::new();
            tree_pids.insert(self.app_pid);

            let mut added = true;
            while added {
                added = false;
                for p in &procs {
                    if !tree_pids.contains(&p.pid) && tree_pids.contains(&p.parent_pid) {
                        tree_pids.insert(p.pid);
                        added = true;
                    }
                }
            }

            let mut app_bytes: u64 = 0;
            let mut webview_bytes: u64 = 0;
            let mut webview_count: usize = 0;

            for pid in &tree_pids {
                if let Some((priv_mem, _ws_mem)) = get_process_memory(*pid) {
                    if *pid == self.app_pid {
                        app_bytes = app_bytes.saturating_add(priv_mem);
                    } else {
                        webview_bytes = webview_bytes.saturating_add(priv_mem);
                        webview_count += 1;
                    }
                }
            }

            let total_bytes = app_bytes.saturating_add(webview_bytes);

            RamUsage {
                total_mb: (total_bytes as f64) / (1024.0 * 1024.0),
                app_mb: (app_bytes as f64) / (1024.0 * 1024.0),
                webview_mb: (webview_bytes as f64) / (1024.0 * 1024.0),
                process_count: tree_pids.len(),
                webview_count,
            }
        }
        #[cfg(not(windows))]
        {
            RamUsage::default()
        }
    }

    pub fn diagnostic(&mut self) -> Vec<ProcessDiagInfo> {
        #[cfg(windows)]
        {
            let procs = snapshot_processes();
            let mut tree_pids = HashSet::new();
            tree_pids.insert(self.app_pid);

            let mut added = true;
            while added {
                added = false;
                for p in &procs {
                    if !tree_pids.contains(&p.pid) && tree_pids.contains(&p.parent_pid) {
                        tree_pids.insert(p.pid);
                        added = true;
                    }
                }
            }

            let mut list = Vec::new();
            for pid in &tree_pids {
                let name = procs
                    .iter()
                    .find(|p| p.pid == *pid)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| {
                        if *pid == self.app_pid {
                            "Oxygen.exe".to_string()
                        } else {
                            "unknown".to_string()
                        }
                    });
                let parent_pid = procs.iter().find(|p| p.pid == *pid).map(|p| p.parent_pid);
                let (priv_bytes, ws_bytes) = get_process_memory(*pid).unwrap_or((0, 0));

                list.push(ProcessDiagInfo {
                    pid: *pid,
                    name,
                    parent_pid,
                    private_mb: (priv_bytes as f64) / (1024.0 * 1024.0),
                    working_set_mb: (ws_bytes as f64) / (1024.0 * 1024.0),
                });
            }
            list.sort_by_key(|p| p.pid);
            list
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }
}

static MONITOR: OnceLock<Mutex<RamMonitor>> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn get_tree_pids(app_pid: u32) -> HashSet<u32> {
    #[cfg(windows)]
    {
        let procs = snapshot_processes();
        let mut tree_pids = HashSet::new();
        tree_pids.insert(app_pid);
        let mut added = true;
        while added {
            added = false;
            for p in &procs {
                if !tree_pids.contains(&p.pid) && tree_pids.contains(&p.parent_pid) {
                    tree_pids.insert(p.pid);
                    added = true;
                }
            }
        }
        tree_pids
    }
    #[cfg(not(windows))]
    {
        let mut s = HashSet::new();
        s.insert(app_pid);
        s
    }
}

fn get_monitor() -> &'static Mutex<RamMonitor> {
    MONITOR.get_or_init(|| Mutex::new(RamMonitor::new()))
}

#[tauri::command]
pub async fn ram_usage() -> RamUsage {
    let mut monitor = get_monitor().lock().unwrap();
    monitor.collect()
}

#[tauri::command]
pub async fn ram_diagnostic() -> Vec<ProcessDiagInfo> {
    let mut monitor = get_monitor().lock().unwrap();
    monitor.diagnostic()
}

pub fn start_monitoring(app_handle: tauri::AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        while RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(2));
            if !RUNNING.load(Ordering::SeqCst) {
                break;
            }

            let usage = {
                let mut monitor = get_monitor().lock().unwrap();
                monitor.collect()
            };

            if let Some(window) = app_handle.get_window("main") {
                let _ = window.emit("ram-update", &usage);
            } else {
                break;
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
}

pub fn stop_monitoring() {
    RUNNING.store(false, Ordering::SeqCst);
}
