use std::collections::BTreeMap;
use std::ffi::CStr;
use std::process::Command;
use std::{io, mem};

// ── proc_pidinfo flavors ────────────────────────────────────────────────────
const PROC_PIDLISTFDS: i32 = 1;
const PROC_PIDREGIONPATHINFO: i32 = 8;

// ── proc_pidfdinfo flavors ──────────────────────────────────────────────────
const PROC_PIDFDVNODEPATHINFO: i32 = 2;

// ── FD types ────────────────────────────────────────────────────────────────
const PROX_FDTYPE_VNODE: u32 = 1;

const MAXPATHLEN: usize = 1024;
const MIN_MODEL_BYTES: u64 = 50 * 1024 * 1024;
const MAX_REGION_ITERATIONS: usize = 200_000;

const MODEL_EXTENSIONS: &[&str] = &[".safetensors", ".gguf", ".bin"];

// ── FFI ─────────────────────────────────────────────────────────────────────
unsafe extern "C" {
    fn proc_pidinfo(
        pid: i32,
        flavor: i32,
        arg: u64,
        buffer: *mut libc::c_void,
        buffersize: i32,
    ) -> i32;

    fn proc_pidfdinfo(
        pid: i32,
        fd: i32,
        flavor: i32,
        buffer: *mut libc::c_void,
        buffersize: i32,
    ) -> i32;
}

// ── Structs matching sys/proc_info.h ────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcFdInfo {
    proc_fd: i32,
    proc_fdtype: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcFileInfo {
    fi_openflags: u32,
    fi_status: u32,
    fi_offset: i64,
    fi_type: i32,
    fi_guardflags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VinfoStat {
    vst_dev: u32,
    vst_mode: u16,
    vst_nlink: u16,
    vst_ino: u64,
    vst_uid: u32,
    vst_gid: u32,
    vst_atime: i64,
    vst_atimensec: i64,
    vst_mtime: i64,
    vst_mtimensec: i64,
    vst_ctime: i64,
    vst_ctimensec: i64,
    vst_birthtime: i64,
    vst_birthtimensec: i64,
    vst_size: i64,
    vst_blocks: i64,
    vst_blksize: i32,
    vst_flags: u32,
    vst_gen: u32,
    vst_rdev: u32,
    vst_qspare: [i64; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Fsid {
    val: [i32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VnodeInfo {
    vi_stat: VinfoStat,
    vi_type: i32,
    vi_pad: i32,
    vi_fsid: Fsid,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VnodeInfoPath {
    vip_vi: VnodeInfo,
    vip_path: [u8; MAXPATHLEN],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VnodeFdInfoWithPath {
    pfi: ProcFileInfo,
    pvip: VnodeInfoPath,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcRegionInfo {
    pri_protection: u32,
    pri_max_protection: u32,
    pri_inheritance: u32,
    pri_flags: u32,
    pri_offset: u64,
    pri_behavior: u32,
    pri_user_wired_count: u32,
    pri_user_tag: u32,
    pri_pages_resident: u32,
    pri_pages_shared_now_private: u32,
    pri_pages_swapped_out: u32,
    pri_pages_dirtied: u32,
    pri_ref_count: u32,
    pri_shadow_depth: u32,
    pri_share_mode: u32,
    pri_private_pages_resident: u32,
    pri_shared_pages_resident: u32,
    pri_obj_id: u32,
    pri_depth: u32,
    pri_address: u64,
    pri_size: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcRegionWithPathInfo {
    prp_prinfo: ProcRegionInfo,
    prp_vip: VnodeInfoPath,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn page_size() -> u64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}

fn extract_path(raw: &[u8; MAXPATHLEN]) -> Option<String> {
    CStr::from_bytes_until_nul(raw.as_slice())
        .ok()
        .and_then(|cs| cs.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn is_model_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    MODEL_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

fn last_errno() -> io::Error {
    io::Error::last_os_error()
}

// ── Layer 1: open FDs ───────────────────────────────────────────────────────

struct FdEntry {
    path: String,
    size: u64,
}

fn probe_open_fds(pid: i32) -> Vec<FdEntry> {
    let fd_info_size = mem::size_of::<ProcFdInfo>() as i32;
    let mut buf_size: i32 = fd_info_size * 256;
    let mut buf: Vec<u8> = vec![0u8; buf_size as usize];

    let ret = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDLISTFDS,
            0,
            buf.as_mut_ptr().cast(),
            buf_size,
        )
    };
    if ret <= 0 {
        eprintln!("  PROC_PIDLISTFDS failed: {}", last_errno());
        return Vec::new();
    }

    // If buffer was too small, retry with returned size + margin
    if ret >= buf_size {
        buf_size = ret + fd_info_size * 32;
        buf.resize(buf_size as usize, 0);
        let ret2 = unsafe {
            proc_pidinfo(
                pid,
                PROC_PIDLISTFDS,
                0,
                buf.as_mut_ptr().cast(),
                buf_size,
            )
        };
        if ret2 <= 0 {
            eprintln!("  PROC_PIDLISTFDS retry failed: {}", last_errno());
            return Vec::new();
        }
    }

    let count = ret as usize / mem::size_of::<ProcFdInfo>();
    let fd_infos: &[ProcFdInfo] =
        unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), count) };

    let mut results = Vec::new();
    let vnode_size = mem::size_of::<VnodeFdInfoWithPath>() as i32;

    for fdi in fd_infos {
        if fdi.proc_fdtype != PROX_FDTYPE_VNODE {
            continue;
        }

        let mut vnode_buf = vec![0u8; vnode_size as usize];
        let ret = unsafe {
            proc_pidfdinfo(
                pid,
                fdi.proc_fd,
                PROC_PIDFDVNODEPATHINFO,
                vnode_buf.as_mut_ptr().cast(),
                vnode_size,
            )
        };
        if ret < vnode_size {
            continue;
        }

        let info: &VnodeFdInfoWithPath = unsafe { &*vnode_buf.as_ptr().cast() };

        if let Some(path) = extract_path(&info.pvip.vip_path) {
            let file_size = info.pvip.vip_vi.vi_stat.vst_size as u64;
            if is_model_file(&path) && file_size >= MIN_MODEL_BYTES {
                results.push(FdEntry {
                    path,
                    size: file_size,
                });
            }
        }
    }

    results
}

// ── Layer 2: VM region walking ──────────────────────────────────────────────

struct RegionEntry {
    path: String,
    region_bytes: u64,
    resident_pages: u32,
}

fn probe_vm_regions(pid: i32) -> Vec<RegionEntry> {
    let buf_size = mem::size_of::<ProcRegionWithPathInfo>() as i32;
    let mut buf = vec![0u8; buf_size as usize];
    let mut address: u64 = 0;
    let mut raw: Vec<RegionEntry> = Vec::new();

    for _ in 0..MAX_REGION_ITERATIONS {
        buf.fill(0);
        let ret = unsafe {
            proc_pidinfo(
                pid,
                PROC_PIDREGIONPATHINFO,
                address,
                buf.as_mut_ptr().cast(),
                buf_size,
            )
        };
        if ret < buf_size {
            break;
        }

        let info: &ProcRegionWithPathInfo = unsafe { &*buf.as_ptr().cast() };
        let ri = &info.prp_prinfo;

        if ri.pri_size == 0 {
            break;
        }

        if let Some(path) = extract_path(&info.prp_vip.vip_path) {
            if is_model_file(&path) {
                raw.push(RegionEntry {
                    path,
                    region_bytes: ri.pri_size,
                    resident_pages: ri.pri_pages_resident,
                });
            }
        }

        address = ri.pri_address.saturating_add(ri.pri_size);
        if address == 0 {
            break;
        }
    }

    // Aggregate by path: sum region sizes and resident pages
    let mut agg: BTreeMap<String, (u64, u32)> = BTreeMap::new();
    for e in &raw {
        let entry = agg.entry(e.path.clone()).or_default();
        entry.0 += e.region_bytes;
        entry.1 += e.resident_pages;
    }

    agg.into_iter()
        .filter(|(_, (total, _))| *total >= MIN_MODEL_BYTES)
        .map(|(path, (region_bytes, resident_pages))| RegionEntry {
            path,
            region_bytes,
            resident_pages,
        })
        .collect()
}

// ── Layer 3: lsof reference ─────────────────────────────────────────────────

fn probe_lsof(pid: i32) -> Vec<String> {
    let output = match Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            eprintln!("  lsof failed to start: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        eprintln!(
            "  lsof exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .filter(|path| is_model_file(path))
        .map(String::from)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

// ── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let pid: i32 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Usage: probe-fds <PID>");
            std::process::exit(1);
        })
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("Invalid PID: {e}");
            std::process::exit(1);
        });

    let ps = page_size();
    println!("Probing PID {pid}  (page size: {ps} bytes)\n");

    // Layer 1
    println!("=== Layer 1: Open FDs (PROC_PIDLISTFDS → PROC_PIDFDVNODEPATHINFO) ===");
    let fd_results = probe_open_fds(pid);
    if fd_results.is_empty() {
        println!("  (none)");
    }
    for e in &fd_results {
        println!("  {}  ({:.1} MB)", e.path, e.size as f64 / 1_048_576.0);
    }
    println!();

    // Layer 2
    println!("=== Layer 2: VM Regions (PROC_PIDREGIONPATHINFO) ===");
    let region_results = probe_vm_regions(pid);
    if region_results.is_empty() {
        println!("  (none)");
    }
    for e in &region_results {
        let resident_bytes = e.resident_pages as u64 * ps;
        let pct = if e.region_bytes > 0 {
            resident_bytes as f64 / e.region_bytes as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "  {}  (mapped: {:.1} MB  resident: {:.1} MB  {:.0}%)",
            e.path,
            e.region_bytes as f64 / 1_048_576.0,
            resident_bytes as f64 / 1_048_576.0,
            pct,
        );
    }
    println!();

    // Layer 3
    println!("=== Layer 3: lsof (reference) ===");
    let lsof_results = probe_lsof(pid);
    if lsof_results.is_empty() {
        println!("  (none)");
    }
    for path in &lsof_results {
        println!("  {path}");
    }
    println!();

    // Dedup summary
    let mut all: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for e in &fd_results {
        all.entry(e.path.clone()).or_default().push("fd");
    }
    for e in &region_results {
        all.entry(e.path.clone()).or_default().push("region");
    }
    for p in &lsof_results {
        all.entry(p.clone()).or_default().push("lsof");
    }

    println!("=== Summary ===");
    if all.is_empty() {
        println!("  No model files detected.");
    }
    for (path, sources) in &all {
        println!("  [{}]  {path}", sources.join("+"));
    }
}
