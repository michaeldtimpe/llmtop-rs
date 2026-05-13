use std::ffi::CStr;
use std::mem;

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

pub struct DetectedFile {
    pub path: String,
    pub size_bytes: u64,
    pub resident_bytes: u64,
}

pub fn detect_model_files(pid: i32, page_size: u64, extensions: &[&str], min_bytes: u64) -> Vec<DetectedFile> {
    let is_match = |p: &str| {
        let Some(ext) = std::path::Path::new(p).extension().and_then(|e| e.to_str()) else {
            return false;
        };
        let ext = ext.to_ascii_lowercase();
        extensions.iter().any(|e| *e == ext)
    };

    let mut results: std::collections::BTreeMap<String, (u64, u64)> = std::collections::BTreeMap::new();

    // Layer 1: open FDs
    let fd_sz = mem::size_of::<ProcFdInfo>() as i32;
    let mut buf = vec![0u8; (fd_sz * 1024) as usize];
    let ret = unsafe { proc_pidinfo(pid, PROC_PIDLISTFDS, 0, buf.as_mut_ptr().cast(), buf.len() as i32) };
    if ret > 0 {
        if ret >= buf.len() as i32 {
            buf.resize((ret + fd_sz * 32) as usize, 0);
            let ret2 = unsafe { proc_pidinfo(pid, PROC_PIDLISTFDS, 0, buf.as_mut_ptr().cast(), buf.len() as i32) };
            if ret2 > 0 {
                let count = ret2 as usize / mem::size_of::<ProcFdInfo>();
                enumerate_fds(pid, &buf, count, &is_match, min_bytes, &mut results);
            }
        } else {
            let count = ret as usize / mem::size_of::<ProcFdInfo>();
            enumerate_fds(pid, &buf, count, &is_match, min_bytes, &mut results);
        }
    }

    // Layer 2: VM regions
    let reg_sz = mem::size_of::<ProcRegionWithPathInfo>() as i32;
    let mut rbuf = vec![0u8; reg_sz as usize];
    let mut addr: u64 = 0;
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
        if let Some(path) = extract_path(&info.prp_vip.vip_path) {
            if is_match(&path) {
                let entry = results.entry(path).or_default();
                entry.0 += ri.pri_size;
                entry.1 += ri.pri_pages_resident as u64 * page_size;
            }
        }
        addr = ri.pri_address.saturating_add(ri.pri_size);
        if addr == 0 {
            break;
        }
    }

    results
        .into_iter()
        .filter(|(_, (sz, _))| *sz >= min_bytes)
        .map(|(path, (size_bytes, resident_bytes))| DetectedFile {
            path,
            size_bytes,
            resident_bytes,
        })
        .collect()
}

fn enumerate_fds(
    pid: i32,
    buf: &[u8],
    count: usize,
    is_match: &dyn Fn(&str) -> bool,
    min_bytes: u64,
    results: &mut std::collections::BTreeMap<String, (u64, u64)>,
) {
    let fds: &[ProcFdInfo] = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast(), count) };
    let vnode_sz = mem::size_of::<VnodeFdInfoWithPath>() as i32;
    let mut vbuf = vec![0u8; vnode_sz as usize];

    for fdi in fds.iter().filter(|f| f.proc_fdtype == PROX_FDTYPE_VNODE) {
        let r = unsafe {
            proc_pidfdinfo(
                pid,
                fdi.proc_fd,
                PROC_PIDFDVNODEPATHINFO,
                vbuf.as_mut_ptr().cast(),
                vnode_sz,
            )
        };
        if r >= vnode_sz {
            let info: &VnodeFdInfoWithPath = unsafe { &*vbuf.as_ptr().cast() };
            if let Some(path) = extract_path(&info.pvip.vip_path) {
                let file_size = info.pvip.vip_vi.vi_stat.vst_size as u64;
                if is_match(&path) && file_size >= min_bytes {
                    let entry = results.entry(path).or_default();
                    if entry.0 == 0 {
                        entry.0 = file_size;
                    }
                }
            }
        }
    }
}
