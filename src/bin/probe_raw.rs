use std::ffi::CStr;
use std::{io, mem};

const PROC_PIDLISTFDS: i32 = 1;
const PROC_PIDREGIONPATHINFO: i32 = 8;
const PROC_PIDFDVNODEPATHINFO: i32 = 2;
const PROX_FDTYPE_VNODE: u32 = 1;
const MAXPATHLEN: usize = 1024;
const MAX_REGION_ITERATIONS: usize = 200_000;

unsafe extern "C" {
    fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buf: *mut libc::c_void, sz: i32) -> i32;
    fn proc_pidfdinfo(pid: i32, fd: i32, flavor: i32, buf: *mut libc::c_void, sz: i32) -> i32;
}

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

fn extract_path(raw: &[u8; MAXPATHLEN]) -> Option<String> {
    CStr::from_bytes_until_nul(raw.as_slice())
        .ok()
        .and_then(|cs| cs.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn main() {
    let pid: i32 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("Usage: probe-raw <PID>");
            std::process::exit(1);
        })
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("Invalid PID: {e}");
            std::process::exit(1);
        });

    // Layer 1: FDs
    let fd_sz = mem::size_of::<ProcFdInfo>() as i32;
    let mut buf = vec![0u8; (fd_sz * 1024) as usize];
    let ret = unsafe { proc_pidinfo(pid, PROC_PIDLISTFDS, 0, buf.as_mut_ptr().cast(), buf.len() as i32) };
    if ret <= 0 {
        eprintln!("PROC_PIDLISTFDS failed: {}", io::Error::last_os_error());
    } else {
        let count = ret as usize / mem::size_of::<ProcFdInfo>();
        let fds: &[ProcFdInfo] = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), count) };
        let vnode_count = fds.iter().filter(|f| f.proc_fdtype == PROX_FDTYPE_VNODE).count();
        println!("FDs: {count} total, {vnode_count} vnodes");

        let vnode_sz = mem::size_of::<VnodeFdInfoWithPath>() as i32;
        let mut vbuf = vec![0u8; vnode_sz as usize];
        let mut shown = 0;
        for fdi in fds.iter().filter(|f| f.proc_fdtype == PROX_FDTYPE_VNODE) {
            let r = unsafe {
                proc_pidfdinfo(pid, fdi.proc_fd, PROC_PIDFDVNODEPATHINFO, vbuf.as_mut_ptr().cast(), vnode_sz)
            };
            if r >= vnode_sz {
                let info: &VnodeFdInfoWithPath = unsafe { &*vbuf.as_ptr().cast() };
                if let Some(path) = extract_path(&info.pvip.vip_path) {
                    let sz = info.pvip.vip_vi.vi_stat.vst_size;
                    if shown < 10 {
                        println!("  fd={}: {}  ({sz} bytes)", fdi.proc_fd, path);
                    }
                    shown += 1;
                }
            }
        }
        if shown > 10 {
            println!("  ... and {} more", shown - 10);
        }
    }

    // Layer 2: regions
    println!("\nVM Regions:");
    let reg_sz = mem::size_of::<ProcRegionWithPathInfo>() as i32;
    let mut rbuf = vec![0u8; reg_sz as usize];
    let mut addr: u64 = 0;
    let mut total = 0;
    let mut with_path = 0;
    let mut shown = 0;
    for _ in 0..MAX_REGION_ITERATIONS {
        rbuf.fill(0);
        let r = unsafe { proc_pidinfo(pid, PROC_PIDREGIONPATHINFO, addr, rbuf.as_mut_ptr().cast(), reg_sz) };
        if r < reg_sz {
            break;
        }
        let info: &ProcRegionWithPathInfo = unsafe { &*rbuf.as_ptr().cast() };
        let ri = &info.prp_prinfo;
        if ri.pri_size == 0 {
            break;
        }
        total += 1;
        if let Some(path) = extract_path(&info.prp_vip.vip_path) {
            with_path += 1;
            if shown < 10 {
                println!(
                    "  0x{:016x} +{:>10}  res={:>6} pages  {}",
                    ri.pri_address, ri.pri_size, ri.pri_pages_resident, path
                );
            }
            shown += 1;
        }
        addr = ri.pri_address.saturating_add(ri.pri_size);
        if addr == 0 {
            break;
        }
    }
    if shown > 10 {
        println!("  ... and {} more with paths", shown - 10);
    }
    println!("Total regions: {total}, with paths: {with_path}");
}
