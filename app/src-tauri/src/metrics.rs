//! Process measurements for `--self-test` and the benchmarks.

/// Peak resident memory of this process so far, in bytes.
#[cfg(unix)]
pub fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage fills the struct we pass; RUSAGE_SELF is always valid.
    let usage = unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return 0;
        }
        usage.assume_init()
    };
    let max = usage.ru_maxrss.max(0) as u64;
    // macOS reports bytes, Linux reports kilobytes.
    if cfg!(target_os = "macos") {
        max
    } else {
        max * 1024
    }
}

/// Peak working set of this process so far, in bytes.
#[cfg(windows)]
pub fn peak_rss_bytes() -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: the pseudo-handle from GetCurrentProcess needs no closing, and
    // `counters` is a correctly sized, writable struct.
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    if ok == 0 {
        0
    } else {
        counters.PeakWorkingSetSize as u64
    }
}
