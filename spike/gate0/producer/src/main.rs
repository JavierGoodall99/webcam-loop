//! THROWAWAY (Gate 0). Writes solid-colour frames with the counter strip into the shared section.
//! Usage: gate0-producer [--name Global\gate0-vcam-frames] [--mode create|open] [--fps 30]
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use std::{env, thread};

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows::Win32::System::Memory::{CreateFileMappingW, MapViewOfFile, OpenFileMappingW, FILE_MAP_ALL_ACCESS, PAGE_READWRITE};
use windows::Win32::System::SystemInformation::GetTickCount64;

const W: u32 = 1280;
const H: u32 = 720;
const HEADER: usize = 64;
const LEN: usize = HEADER + (W * H * 4) as usize;
const MAGIC: u32 = 0x504C_4357;
const BLOCK: usize = 8;
const BITS: usize = 48;
const MARKER_LIVE: u64 = 0xA5C3;

struct Section {
    _handle: HANDLE,
    base: *mut u8,
}

fn arg(name: &str, default: &str) -> String {
    let a: Vec<String> = env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1)).cloned().unwrap_or_else(|| default.to_string())
}

fn create_section(name: &HSTRING) -> windows::core::Result<Section> {
    unsafe {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        // Everyone: generic all. Spike only.
        ConvertStringSecurityDescriptorToSecurityDescriptorW(w!("D:(A;;GA;;;WD)"), SDDL_REVISION_1, &mut sd, None)?;
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: false.into(),
        };
        let handle = CreateFileMappingW(INVALID_HANDLE_VALUE, Some(&sa as *const _), PAGE_READWRITE, 0, LEN as u32, name)?;
        map(handle)
    }
}

fn open_section(name: &HSTRING) -> windows::core::Result<Section> {
    unsafe { map(OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, name)?) }
}

unsafe fn map(handle: HANDLE) -> windows::core::Result<Section> {
    let view = MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, LEN);
    if view.Value.is_null() {
        return Err(windows::core::Error::from_win32());
    }
    Ok(Section { _handle: handle, base: view.Value as *mut u8 })
}

unsafe fn publish(s: &Section, counter: u64) {
    let seq = &*(s.base.add(24) as *const AtomicU32);
    seq.fetch_add(1, Ordering::AcqRel); // odd: writing
    let put32 = |o: usize, v: u32| (s.base.add(o) as *mut u32).write_unaligned(v);
    put32(0, MAGIC);
    put32(4, 1);
    put32(8, W);
    put32(12, H);
    put32(16, W * 4);
    put32(20, 1);
    (s.base.add(32) as *mut u64).write_unaligned(counter);
    (s.base.add(40) as *mut u64).write_unaligned(GetTickCount64());

    let pixels = std::slice::from_raw_parts_mut(s.base.add(HEADER), (W * H * 4) as usize);
    let hue = (counter % 256) as u8;
    for px in pixels.chunks_exact_mut(4) {
        px[0] = hue;
        px[1] = 255 - hue;
        px[2] = 128;
        px[3] = 255; // B, G, R, A
    }
    let value: u64 = (MARKER_LIVE << 32) | (counter & 0xFFFF_FFFF);
    for i in 0..BITS {
        let v = if (value >> (BITS - 1 - i)) & 1 == 1 { 255u8 } else { 0u8 };
        for y in 0..BLOCK {
            for x in (i * BLOCK)..((i + 1) * BLOCK) {
                let o = (y * W as usize + x) * 4;
                pixels[o] = v;
                pixels[o + 1] = v;
                pixels[o + 2] = v;
                pixels[o + 3] = 255;
            }
        }
    }
    seq.fetch_add(1, Ordering::Release); // even: done
}

fn main() {
    let name = HSTRING::from(arg("--name", "Global\\gate0-vcam-frames"));
    let mode = arg("--mode", "create");
    let fps: u64 = arg("--fps", "30").parse().unwrap_or(30);

    let section = loop {
        let r = if mode == "open" { open_section(&name) } else { create_section(&name) };
        match r {
            Ok(s) => break s,
            Err(e) if mode == "open" => {
                eprintln!("waiting for section: {e}");
                thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                eprintln!("cannot create section {name}: {e}");
                std::process::exit(1);
            }
        }
    };
    eprintln!("producer running, mode={mode}, {W}x{H} @ {fps} fps");

    let interval = Duration::from_micros(1_000_000 / fps);
    let mut next = Instant::now();
    let mut counter: u64 = 0;
    loop {
        unsafe { publish(&section, counter) };
        counter += 1;
        next += interval;
        if let Some(d) = next.checked_duration_since(Instant::now()) {
            thread::sleep(d);
        }
    }
}
