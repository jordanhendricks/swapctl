// constants and structs from usr/src/uts/common/sys/swap.h

// swapctl(2) commands
const SC_ADD: i32 = 0x1;
const SC_LIST: i32 = 0x2;
const _SC_REMOVE: i32 = 0x3;
const SC_GETNSWP: i32 = 0x4;
const SC_AINFO: i32 = 0x5;

// swapctl(2)
extern "C" {
    fn swapctl(cmd: i32, arg: *mut libc::c_void) -> i32;
}

// SC_ADD / SC_REMOVE arg
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct swapres {
    sr_name: *const libc::c_char,
    sr_start: libc::off_t,
    sr_length: libc::off_t,
}

// SC_LIST arg
#[repr(C)]
#[derive(Debug, Clone)]
pub struct swaptbl {
    swt_n: i32,
    swt_ent: [swapent; N_SWAPENTS],
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct swapent {
    ste_path: *const libc::c_char,
    ste_start: libc::off_t,
    ste_length: libc::off_t,
    ste_pages: libc::c_long,
    ste_free: libc::c_long,
    ste_flags: libc::c_long,
}
impl Default for swapent {
    fn default() -> Self {
        Self {
            ste_path: std::ptr::null(),
            ste_start: 0,
            ste_length: 0,
            ste_pages: 0,
            ste_free: 0,
            ste_flags: 0,
        }
    }
}

// The argument for SC_LIST (swaptbl) requires an embedded array in the struct,
// with swt_n entries, each of which requires a pointer to store the path to the
// device.
//
// Ideally, we would want to query the number of swap devices on the system via
// SC_GETNSWP, allocate enough memory for the number of devices, then list the
// swap devices. Creating a generically large array embedded in a struct that
// can be passed to C is a bit of a challenge in safe Rust. So instead, we just
// pick a reasonable max number of devices to list.
//
// We pick a max of 3 devices, somewhat arbitrarily, but log the number of
// swap devices we see regardless. We only ever expect to see 0 or 1 swap
// device(s); if there are more, that is a bug. In this case we log a warning,
// and eventually, we should send an ereport.
const N_SWAPENTS: usize = 3;

unsafe fn swapctl_cmd<T>(cmd: i32, data: Option<*mut T>) -> std::io::Result<u32> {
    assert!(cmd >= 0 && cmd <= SC_AINFO, "invalid swapctl cmd: {cmd}");

    let ptr = match data {
        Some(v) => v as *mut libc::c_void,
        None => std::ptr::null_mut(),
    };

    let res = swapctl(cmd, ptr);
    if res == -1 {
        // TODO: log message
        // TODO: custom error
        return Err(std::io::Error::last_os_error());
    }

    Ok(res as u32)
}

pub fn swapctl_get_num_devices() -> std::io::Result<u32> {
    unsafe { swapctl_cmd::<i32>(SC_GETNSWP, None) }
}

// TODO: probably want to return a real Rust struct here
pub fn swapctl_list() -> std::io::Result<(usize, swaptbl)> {
    // statically allocate the array of swapents for SC_LIST
    //
    // see comment on `N_SWAPENTS` for details
    const MAXPATHLEN: usize = libc::PATH_MAX as usize;
    let p1 = [0i8; MAXPATHLEN];
    let p2 = [0i8; MAXPATHLEN];
    let p3 = [0i8; MAXPATHLEN];

    let entries: [swapent; N_SWAPENTS] = [
        swapent {
            ste_path: &p1 as *const libc::c_char,
            ..Default::default()
        },
        swapent {
            ste_path: &p2 as *const libc::c_char,
            ..Default::default()
        },
        swapent {
            ste_path: &p3 as *const libc::c_char,
            ..Default::default()
        },
    ];

    let mut list_req = swaptbl {
        swt_n: N_SWAPENTS as i32,
        swt_ent: entries,
    };

    let n_devices = unsafe { swapctl_cmd(SC_LIST, Some(&mut list_req))? };

    Ok((n_devices as usize, list_req))
}

// TODO: can start be negative (off_t is i64)
pub fn swapctl_add(name: &str, start: u64, length: u64) -> std::io::Result<()> {
    // start and length must be specified in 512-byte blocks
    assert_eq!(start % 512, 0, "start not divisible by 512: {}", start);
    assert_eq!(length % 512, 0, "length not divisible by 512: {}", length);

    // TODO: probably a real error here
    let n = std::ffi::CString::new(name).unwrap();

    let mut add_req = swapres {
        sr_name: n.as_ptr(),
        sr_start: start as libc::off_t,
        sr_length: length as libc::off_t,
    };
    println!("add_req: {:?}", add_req);

    let res = unsafe { swapctl_cmd(SC_ADD, Some(&mut add_req)) }?;
    assert!(res == 0);

    Ok(())
}

fn main() {
    let p = std::ptr::null_mut();
    let r = unsafe { swapctl(SC_GETNSWP, p) };
    println!("swapctl getnswp = {}", r);
    let (n, lr) = swapctl_list().unwrap();
    println!("swapctl listswap = {:?}\n", lr);

    for i in 0..n {
        let e = lr.swt_ent[i as usize];
        let p = unsafe { std::ffi::CStr::from_ptr(e.ste_path) };
        println!(
            "swapfile {:?}: start {:?}, length {:?}, {:?} pages, {:?} free, 0x{:x} flags",
            p, e.ste_start, e.ste_length, e.ste_pages, e.ste_free, e.ste_flags
        );
    }

    // TODO: how to get this path for the zvol?
    //let add = swapctl_add("/dev/zvol/dsk/rpool/testswap", 0, 0);
    println!("add = {:?}", add);
}
