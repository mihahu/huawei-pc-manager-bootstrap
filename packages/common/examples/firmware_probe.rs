//! Load a built patch in this disposable process, without touching PCManager.
use std::{ffi::c_void, mem::transmute};

use anyhow::{ensure, Context};
use common::config::{get_firmware_config, Config};
use injectors::options::InjectOptions;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> isize;
    fn FreeLibrary(module: isize) -> i32;
    fn GetProcAddress(module: isize, name: *const u8) -> *const c_void;
    fn GetSystemFirmwareTable(provider: u32, id: u32, buffer: *mut c_void, size: u32) -> u32;
}

const RSMB: u32 = 0x52534d42;

fn query() -> anyhow::Result<Vec<u8>> {
    let required = unsafe { GetSystemFirmwareTable(RSMB, 0, std::ptr::null_mut(), 0) };
    ensure!(required > 8, "Firmware size query failed");
    let mut data = vec![0; required as usize];
    let actual =
        unsafe { GetSystemFirmwareTable(RSMB, 0, data.as_mut_ptr().cast(), data.len() as u32) };
    ensure!(actual == required, "Firmware size changed unexpectedly");
    Ok(data)
}

fn system_record(data: &[u8]) -> anyhow::Result<&[u8]> {
    ensure!(data.len() >= 8, "Missing raw header");
    let length = u32::from_le_bytes(data[4..8].try_into()?) as usize;
    ensure!(
        length == data.len() - 8,
        "Raw header has an incorrect table length"
    );
    let mut offset = 8;
    while offset < data.len() {
        ensure!(data.len() - offset >= 4, "Truncated record");
        let length = data[offset + 1] as usize;
        ensure!(
            length >= 4 && length <= data.len() - offset,
            "Invalid record length"
        );
        let end = data[offset + length..]
            .windows(2)
            .position(|pair| pair == [0, 0])
            .map(|relative| offset + length + relative + 2)
            .context("Missing record terminator")?;
        if data[offset] == 1 {
            return Ok(&data[offset..end]);
        }
        offset = end;
    }
    anyhow::bail!("Missing system information");
}

fn field(record: &[u8], offset: usize) -> anyhow::Result<&str> {
    ensure!(offset < record[1] as usize, "Missing string field");
    let index = record[offset] as usize;
    ensure!(index > 0, "Empty string field");
    let bytes = record[record[1] as usize..]
        .split(|byte| *byte == 0)
        .nth(index - 1)
        .context("Invalid string index")?;
    Ok(std::str::from_utf8(bytes)?)
}

fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args_os().collect();
    ensure!(
        arguments.len() == 3,
        "Usage: firmware_probe <runtime|core|spawn|injected> <dll-path>"
    );
    if arguments[1] == "spawn" {
        let dll = std::path::Path::new(&arguments[2]).canonicalize()?;
        std::env::set_current_dir(dll.parent().context("DLL has no parent directory")?)?;
        common::common::enable_hook(Some(InjectOptions {
            server_address: None,
            inject_sub_process: true,
            includes_system_process: false,
        }))?;
        let child = std::process::Command::new(std::env::current_exe()?)
            .arg("injected")
            .arg(&dll)
            .output()?;
        ensure!(
            child.status.success(),
            "Injected child failed: {} {}",
            child.status,
            String::from_utf8_lossy(&child.stderr)
        );
        print!("{}", String::from_utf8_lossy(&child.stdout));
        println!("PASS: CreateProcessW child injection and InjectionGuard lifetime");
        return Ok(());
    }
    let original = query()?;
    let name: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        arguments[2].encode_wide().chain([0]).collect()
    };
    let module = if arguments[1] == "injected" {
        0
    } else {
        unsafe { LoadLibraryW(name.as_ptr()) }
    };
    ensure!(
        module != 0 || arguments[1] == "injected",
        "LoadLibraryW failed: {}",
        std::io::Error::last_os_error()
    );
    if arguments[1] == "core" {
        let entry = unsafe { GetProcAddress(module, b"enable_hook\0".as_ptr()) };
        ensure!(!entry.is_null(), "Missing enable_hook export");
        let enable: unsafe extern "system" fn(*const c_void) = unsafe { transmute(entry) };
        unsafe { enable(std::ptr::null()) };
    } else {
        ensure!(
            arguments[1] == "runtime" || arguments[1] == "injected",
            "Unknown probe mode"
        );
        if arguments[1] == "runtime" {
            let address = unsafe { GetProcAddress(module, b"GetFileVersionInfoSizeW\0".as_ptr()) };
            ensure!(!address.is_null(), "Missing version.dll forwarding export");
            let get_version_size: unsafe extern "system" fn(*const u16, *mut u32) -> u32 =
                unsafe { transmute(address) };
            let system_file =
                std::path::PathBuf::from(std::env::var_os("WINDIR").context("Missing WINDIR")?)
                    .join("System32")
                    .join("kernel32.dll");
            let filename: Vec<u16> = {
                use std::os::windows::ffi::OsStrExt;
                system_file.as_os_str().encode_wide().chain([0]).collect()
            };
            let mut ignored = 0;
            ensure!(
                unsafe { get_version_size(filename.as_ptr(), &mut ignored) } > 0,
                "version.dll forwarding did not reach the system implementation"
            );
        }
    }

    let patched = query()?;
    let record = system_record(&patched)?;
    let expected = get_firmware_config().unwrap_or_else(|_| Config::default());
    ensure!(
        field(record, 4)? == expected.manufacturer,
        "Manufacturer was not patched"
    );
    ensure!(
        field(record, 5)? == expected.product_name,
        "Product name was not patched"
    );
    ensure!(
        field(record, 7)? == expected.sn,
        "Serial reference was not patched"
    );
    if record[1] > 25 {
        ensure!(
            field(record, 25)? == expected.sku,
            "SKU reference was not patched"
        );
    }
    let original_record = system_record(&original)?;
    if original_record[1] >= 24 {
        ensure!(
            record[8..24] == original_record[8..24],
            "Original UUID changed"
        );
    }

    let required = patched.len();
    let mut short = vec![0xaa; required - 1];
    let short_result =
        unsafe { GetSystemFirmwareTable(RSMB, 0, short.as_mut_ptr().cast(), short.len() as u32) };
    ensure!(
        short_result as usize == required,
        "Short buffer returned the wrong size"
    );
    ensure!(
        short.iter().all(|byte| *byte == 0xaa),
        "Short buffer was modified"
    );
    let mut oversized = vec![0xaa; required + 64];
    let large_result = unsafe {
        GetSystemFirmwareTable(
            RSMB,
            0,
            oversized.as_mut_ptr().cast(),
            oversized.len() as u32,
        )
    };
    ensure!(
        large_result as usize == required,
        "Oversized query returned the wrong size"
    );
    ensure!(
        oversized[..required] == patched,
        "Oversized query returned different data"
    );
    ensure!(
        oversized[required..].iter().all(|byte| *byte == 0xaa),
        "Buffer tail was overwritten"
    );

    if module != 0 {
        ensure!(unsafe { FreeLibrary(module) } != 0, "FreeLibrary failed");
    }
    ensure!(query()? == patched, "Hooks did not survive FreeLibrary");
    println!(
        "PASS: {} patch ({}) - real firmware, identity, UUID, buffer boundaries, FreeLibrary lifetime",
        arguments[1].to_string_lossy(),
        std::env::consts::ARCH
    );
    Ok(())
}
