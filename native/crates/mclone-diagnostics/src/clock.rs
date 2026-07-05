#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug)]
pub struct MonotonicSample {
    instant: Instant,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug)]
pub struct MonotonicSample;

#[cfg(not(target_arch = "wasm32"))]
impl MonotonicSample {
    pub fn elapsed_ms(self) -> f64 {
        self.instant.elapsed().as_secs_f64() * 1000.0
    }
}

#[cfg(target_arch = "wasm32")]
impl MonotonicSample {
    pub fn elapsed_ms(self) -> f64 {
        0.0
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn monotonic_now() -> Option<MonotonicSample> {
    Some(MonotonicSample {
        instant: Instant::now(),
    })
}

#[cfg(target_arch = "wasm32")]
pub fn monotonic_now() -> Option<MonotonicSample> {
    None
}

#[cfg(all(unix, not(target_arch = "wasm32")))]
pub fn thread_cpu_time_ms() -> Option<f64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `time` is a valid writable pointer for the duration of the call.
    let result = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    if result != 0 {
        return None;
    }
    Some(time.tv_sec as f64 * 1000.0 + time.tv_nsec as f64 / 1_000_000.0)
}

#[cfg(windows)]
pub fn thread_cpu_time_ms() -> Option<f64> {
    use core::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FileTime {
        dw_low_date_time: u32,
        dw_high_date_time: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn GetThreadTimes(
            thread: *mut c_void,
            creation_time: *mut FileTime,
            exit_time: *mut FileTime,
            kernel_time: *mut FileTime,
            user_time: *mut FileTime,
        ) -> i32;
    }

    fn file_time_100ns(time: FileTime) -> u64 {
        ((time.dw_high_date_time as u64) << 32) | time.dw_low_date_time as u64
    }

    let mut creation_time = FileTime {
        dw_low_date_time: 0,
        dw_high_date_time: 0,
    };
    let mut exit_time = creation_time;
    let mut kernel_time = creation_time;
    let mut user_time = creation_time;
    // SAFETY: `GetCurrentThread` returns a pseudo-handle valid in this process, and all
    // FILETIME pointers refer to initialized stack slots for the duration of the call.
    let ok = unsafe {
        GetThreadTimes(
            GetCurrentThread(),
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        )
    };
    if ok == 0 {
        return None;
    }
    let total_100ns = file_time_100ns(kernel_time).saturating_add(file_time_100ns(user_time));
    Some(total_100ns as f64 / 10_000.0)
}

#[cfg(any(target_arch = "wasm32", not(any(unix, windows))))]
pub fn thread_cpu_time_ms() -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_sampler_reports_elapsed_time_when_supported() {
        if let Some(sample) = monotonic_now() {
            assert!(sample.elapsed_ms() >= 0.0);
        }
    }

    #[cfg(all(unix, not(target_arch = "wasm32")))]
    #[test]
    fn unix_thread_cpu_sampler_is_available() {
        assert!(thread_cpu_time_ms().is_some());
    }
}
