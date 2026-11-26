use injectors::{options::InjectOptions, process::ProcessHandle};
use retour::static_detour;
use std::{
    ffi::{c_void, CString},
    mem::transmute,
    slice::{from_raw_parts, from_raw_parts_mut},
    sync::Mutex,
};
use tracing::{error, info, warn};
use widestring::{U16CString, WideCString};
use windows_sys::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{GetLastError, BOOL},
        Security::SECURITY_ATTRIBUTES,
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryW},
            SystemInformation::{GetSystemDirectoryW, FIRMWARE_TABLE_ID, FIRMWARE_TABLE_PROVIDER},
            Threading::{
                ResumeThread, TerminateProcess, CREATE_SUSPENDED, PROCESS_CREATION_FLAGS,
                PROCESS_INFORMATION, STARTUPINFOW,
            },
        },
    },
};

use crate::{
    config::{get_firmware_config, Config},
    smbios::{copy_firmware_table, rewrite_system_info},
};

const GET_MODULE_HANDLE_EX_FLAG_PIN: u32 = 0x0000_0001;
const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 0x0000_0004;

type FnCreateProcessW = unsafe extern "system" fn(
    PCWSTR,
    PWSTR,
    *const SECURITY_ATTRIBUTES,
    *const SECURITY_ATTRIBUTES,
    BOOL,
    PROCESS_CREATION_FLAGS,
    *const c_void,
    PCWSTR,
    *const STARTUPINFOW,
    *mut PROCESS_INFORMATION,
) -> BOOL;
type FnGetSystemFirmwareTable = unsafe extern "system" fn(
    FIRMWARE_TABLE_PROVIDER,
    FIRMWARE_TABLE_ID,
    *mut ::core::ffi::c_void,
    u32,
) -> u32;

static_detour! {
    static HookCreateProcessW: unsafe extern "system" fn(
        PCWSTR,
        PWSTR,
        *const SECURITY_ATTRIBUTES,
        *const SECURITY_ATTRIBUTES,
        BOOL,
        PROCESS_CREATION_FLAGS,
        *const c_void,
        PCWSTR,
        *const STARTUPINFOW,
        *mut PROCESS_INFORMATION
    ) -> BOOL;
  static HookGetSystemFirmwareTable: unsafe extern "system" fn(
        u32,
        u32,
        *mut c_void,
        u32
    ) -> u32;
}

static LIBRARY_NAME: &str = "huawei_pc_manager_bootstrap_core.dll";
static SMBIOS_FIRMWARE_TABLE_PROVIDER: u32 = 1381190978;
static SMBIOS_FIRMWARE_TABLE_ID: u32 = 0;

lazy_static::lazy_static! {
    static ref CUSTOM_SMBIOS_BUFFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);
}

fn is_args_can_hook(
    firmwaretableprovidersignature: FIRMWARE_TABLE_PROVIDER,
    firmwaretableid: FIRMWARE_TABLE_ID,
) -> bool {
    firmwaretableprovidersignature == SMBIOS_FIRMWARE_TABLE_PROVIDER
        && firmwaretableid == SMBIOS_FIRMWARE_TABLE_ID
}

fn get_and_cache_firmware_table() -> anyhow::Result<()> {
    if CUSTOM_SMBIOS_BUFFER
        .lock()
        .map_err(|err| anyhow::anyhow!("Failed to lock custom smbios data buffer: {}", err))?
        .is_some()
    {
        info!("Using cached custom smbios data");
        return Ok(());
    }

    let buffer_size = unsafe {
        HookGetSystemFirmwareTable.call(
            SMBIOS_FIRMWARE_TABLE_PROVIDER,
            SMBIOS_FIRMWARE_TABLE_ID,
            std::ptr::null_mut::<c_void>(),
            0,
        )
    };
    if buffer_size == 0 {
        return Err(anyhow::anyhow!(
            "Failed to get firmware table: {}",
            std::io::Error::last_os_error()
        ));
    }

    let mut buffer: Vec<u8> = vec![0; buffer_size as usize];
    let buffer_size = unsafe {
        HookGetSystemFirmwareTable.call(
            SMBIOS_FIRMWARE_TABLE_PROVIDER,
            SMBIOS_FIRMWARE_TABLE_ID,
            buffer.as_mut_ptr() as *mut c_void,
            buffer.len() as u32,
        )
    };
    if buffer_size == 0 {
        return Err(anyhow::anyhow!(
            "Failed to get firmware table: {}",
            std::io::Error::last_os_error()
        ));
    }
    anyhow::ensure!(
        buffer_size as usize <= buffer.len(),
        "Firmware table grew during the query; retry required"
    );
    buffer.truncate(buffer_size as usize);
    let config = get_firmware_config().unwrap_or_else(|err| {
        warn!("Using default firmware identity: {}", err);
        Config::default()
    });
    let custom = rewrite_system_info(&buffer, &config)?;
    info!(
        "Firmware identity prepared: manufacturer={}, product={}, original_size={}, patched_size={}",
        config.manufacturer,
        config.product_name,
        buffer.len(),
        custom.len()
    );

    CUSTOM_SMBIOS_BUFFER
        .lock()
        .map_err(|err| anyhow::anyhow!("Failed to lock custom smbios data buffer: {}", err))?
        .replace(custom);

    Ok(())
}

fn detour_get_system_firmware_table(
    firmwaretableprovidersignature: FIRMWARE_TABLE_PROVIDER,
    firmwaretableid: FIRMWARE_TABLE_ID,
    pfirmwaretablebuffer: *mut ::core::ffi::c_void,
    buffersize: u32,
) -> u32 {
    let sig_name = get_firmware_table_provider_signature(firmwaretableprovidersignature);
    let id_name = get_firmware_table_provider_signature(firmwaretableid);
    info!(
        "Calling GetSystemFirmwareTable: {}({}), {}({}), 0x{:x}, {}",
        firmwaretableprovidersignature,
        sig_name,
        firmwaretableid,
        id_name,
        pfirmwaretablebuffer as usize,
        buffersize
    );

    if is_args_can_hook(firmwaretableprovidersignature, firmwaretableid) {
        let cache_smbios_buffer_result = get_and_cache_firmware_table();
        if cache_smbios_buffer_result.is_ok() {
            match CUSTOM_SMBIOS_BUFFER.lock() {
                Ok(buffer) => {
                    if let Some(buffer) = buffer.as_ref() {
                        if pfirmwaretablebuffer as usize == 0 {
                            info!("Need {} bytes buffer", buffer.len());
                            return copy_firmware_table(buffer, None);
                        }
                        if (buffersize as usize) < buffer.len() {
                            info!("Firmware buffer too small; need {} bytes", buffer.len());
                            return copy_firmware_table(buffer, None);
                        }
                        let dest: &mut [u8] = unsafe {
                            from_raw_parts_mut(pfirmwaretablebuffer as *mut u8, buffersize as usize)
                        };
                        let size = copy_firmware_table(buffer, Some(dest));
                        info!("Copied {} bytes from firmware buffer", size);
                        return size;
                    }
                    error!("Failed to get custom smbios buffer");
                }
                Err(err) => {
                    error!("Failed to lock custom smbios data buffer: {}", err);
                }
            }
        } else {
            error!(
                "Failed to get and cache firmware table: {}",
                cache_smbios_buffer_result.unwrap_err()
            );
        }
        warn!("Firmware patch unavailable; forwarding the original query");
    }

    unsafe {
        HookGetSystemFirmwareTable.call(
            firmwaretableprovidersignature,
            firmwaretableid,
            pfirmwaretablebuffer,
            buffersize,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn detour_create_process(
    opts: &Option<InjectOptions>,
    app_name: PCWSTR,
    cmd_line: PWSTR,
    proc_attrs: *const SECURITY_ATTRIBUTES,
    th_attrs: *const SECURITY_ATTRIBUTES,
    inherit: BOOL,
    flags: PROCESS_CREATION_FLAGS,
    env: *const c_void,
    cur_dir: PCWSTR,
    startup_info: *const STARTUPINFOW,
    proc_info: *mut PROCESS_INFORMATION,
) -> BOOL {
    unsafe {
        let app_name_string = app_name
            .as_ref()
            .map(|app_name| U16CString::from_ptr_str(app_name).to_string())
            .unwrap_or_else(|| Ok(String::new()));
        let cmd_line_string = cmd_line
            .as_ref()
            .map(|cmd_line| U16CString::from_ptr_str(cmd_line).to_string())
            .unwrap_or_else(|| Ok(String::new()));
        let (app_name_string, cmd_line_string) = match (app_name_string, cmd_line_string) {
            (Ok(a), Ok(b)) => (a, b),
            (a, c) => {
                match (a, c) {
                    (Err(err), _) => {
                        error!("Failed to parse app_name: {err}")
                    }
                    (_, Err(err)) => {
                        error!("Failed to parse cmd_line: {err}")
                    }
                    _ => {}
                }
                return HookCreateProcessW.call(
                    app_name,
                    cmd_line,
                    proc_attrs,
                    th_attrs,
                    inherit,
                    flags,
                    env,
                    cur_dir,
                    startup_info,
                    proc_info,
                );
            }
        };
        info!("CreateProcessW: {} {}", app_name_string, cmd_line_string);

        let flags_with_suspend = CREATE_SUSPENDED | flags;
        let creating_res = HookCreateProcessW.call(
            app_name,
            cmd_line,
            proc_attrs,
            th_attrs,
            inherit,
            flags_with_suspend,
            env,
            cur_dir,
            startup_info,
            proc_info,
        );

        if creating_res != 0 {
            info!("New process id: {:?}", (*proc_info).dwProcessId);
            if cmd_line_string.contains("isSupportDevice") {
                info!("Command line contains isSupportDevice, exit with 1");
                TerminateProcess((*proc_info).hProcess, 1);
                return creating_res;
            }
            if cmd_line_string.contains("IsSupportBaZhang") {
                info!("Command line contains IsSupportBaZhang, exit with 2");
                TerminateProcess((*proc_info).hProcess, 2);
                return creating_res;
            }
            let should_inject = opts
                .as_ref()
                .map(|opts| {
                    should_inject_process(opts, app_name_string.as_str(), cmd_line_string.as_str())
                })
                .unwrap_or(true);
            if should_inject {
                let process = ProcessHandle::from_handle((*proc_info).hProcess);
                let inject_result = process.inject_to_process(opts, LIBRARY_NAME);
                if let Err(err) = inject_result {
                    // The dependency labels architecture mismatches "Bincode Error".
                    warn!(
                        "inject_to_process error for pid {}: {:?}",
                        (*proc_info).dwProcessId,
                        err
                    );
                }
            } else {
                info!("Skip system process.");
            }
            if flags & CREATE_SUSPENDED == 0 && ResumeThread((*proc_info).hThread) == u32::MAX {
                warn!("ResumeThread error: {}", GetLastError());
            }
        } else {
            warn!("CreateProcessW failed: {}", GetLastError());
        }

        creating_res
    }
}

pub fn enable_hook(opts: Option<InjectOptions>) -> anyhow::Result<()> {
    let inject_sub_process = opts
        .as_ref()
        .map(|opts| opts.inject_sub_process)
        .unwrap_or(false);
    unsafe {
        let fp_get_system_firmware_table: FnGetSystemFirmwareTable =
            transmute(get_proc_address("GetSystemFirmwareTable", "kernel32.dll")?);
        info!(
            "Got GetSystemFirmwareTable: 0x{:x}",
            fp_get_system_firmware_table as usize
        );

        HookGetSystemFirmwareTable.initialize(
            fp_get_system_firmware_table,
            detour_get_system_firmware_table,
        )?;
        info!("HookGetSystemFirmwareTable initialized");

        if inject_sub_process {
            let fp_create_process: FnCreateProcessW =
                transmute(get_proc_address("CreateProcessW", "kernel32.dll")?);
            info!("Got CreateProcessW: 0x{:x}", fp_create_process as usize);
            let opts = Box::leak(Box::new(opts));
            HookCreateProcessW.initialize(
                fp_create_process,
                |app_name,
                 cmd_line,
                 proc_attrs,
                 th_attrs,
                 inherit,
                 flags,
                 env,
                 cur_dir,
                 startup_info,
                 proc_info| {
                    detour_create_process(
                        opts,
                        app_name,
                        cmd_line,
                        proc_attrs,
                        th_attrs,
                        inherit,
                        flags,
                        env,
                        cur_dir,
                        startup_info,
                        proc_info,
                    )
                },
            )?;
            info!("HookCreateProcessW initialized");
        }
        HookGetSystemFirmwareTable.enable()?;
        info!("HookGetSystemFirmwareTable enabled");

        if inject_sub_process {
            HookCreateProcessW.enable()?;
            info!("HookCreateProcessW enabled");
        }
    }

    Ok(())
}

fn should_inject_process(opts: &InjectOptions, app_name: &str, cmd_line: &str) -> bool {
    opts.includes_system_process
        || (if !app_name.trim().is_empty() {
            !check_path_is_system(app_name)
        } else {
            !check_path_is_system(cmd_line)
        })
}

type FnGetModuleHandleExW = unsafe extern "system" fn(u32, PCWSTR, *mut *mut c_void) -> BOOL;

/// Pin the module containing `address_in_module` so that a later `FreeLibrary`
/// cannot actually unload it. The hook callbacks installed by `enable_hook`
/// live in this module's code section; unloading the module would leave the
/// inline detours pointing into freed memory and crash the host process with
/// STATUS_ACCESS_VIOLATION (0xc0000005). `address_in_module` must point into
/// the module to pin (e.g. the address of the exported `enable_hook` function).
pub unsafe fn pin_module(address_in_module: *const c_void) -> anyhow::Result<()> {
    let get_module_handle_ex_w: FnGetModuleHandleExW =
        transmute(get_proc_address("GetModuleHandleExW", "kernel32.dll")?);
    let mut module_handle: *mut c_void = std::ptr::null_mut();
    let ok = get_module_handle_ex_w(
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
        address_in_module as PCWSTR,
        &mut module_handle,
    );
    if ok == 0 {
        return Err(anyhow::anyhow!(
            "GetModuleHandleExW failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    if module_handle.is_null() {
        return Err(anyhow::anyhow!("GetModuleHandleExW returned null handle"));
    }
    info!("Pinned module: 0x{:x}", module_handle as usize);
    Ok(())
}

unsafe fn get_proc_address(
    proc_name: &str,
    module_name: &str,
) -> anyhow::Result<unsafe extern "system" fn() -> isize> {
    let module_name_cstr = WideCString::from_str(module_name)?;
    let h_inst = LoadLibraryW(module_name_cstr.as_ptr());
    if h_inst == 0 {
        return Err(anyhow::anyhow!(
            "LoadLibraryW failed: {}",
            std::io::Error::last_os_error()
        ));
    }

    let proc_name_cstr = CString::new(proc_name)?;
    GetProcAddress(h_inst, proc_name_cstr.as_ptr() as _).ok_or_else(|| {
        anyhow::anyhow!("GetProcAddress failed: {}", std::io::Error::last_os_error())
    })
}

fn check_path_is_system(path: &str) -> bool {
    let mut path_buffer = [0; 4096];
    let size = unsafe { GetSystemDirectoryW(path_buffer.as_mut_ptr(), path_buffer.len() as _) };
    if size > 0 {
        let sys_dir = unsafe { WideCString::from_ptr(path_buffer.as_ptr(), size as _) }
            .ok()
            .and_then(|s| s.to_string().ok());
        info!("System directory: {:?}", sys_dir);
        if let Some(sys_dir) = sys_dir {
            let slash_sys_dir = sys_dir.replace('\\', "/").to_ascii_lowercase();
            let slash_path = path.replace('\\', "/").to_ascii_lowercase();
            return slash_path.starts_with(&slash_sys_dir)
                || (slash_path.starts_with('"') && slash_path[1..].starts_with(&slash_sys_dir));
        }
    }
    false
}

fn get_firmware_table_provider_signature(firmwaretableprovidersignature: u32) -> String {
    let mut sig_name_bytes = unsafe {
        from_raw_parts(
            &firmwaretableprovidersignature as *const u32 as *const u8,
            4,
        )
    }
    .to_vec();
    sig_name_bytes.reverse();
    String::from_utf8(sig_name_bytes).unwrap_or_else(|e| format!("Error({})", e))
}
