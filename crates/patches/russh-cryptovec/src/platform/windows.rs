use libc::c_void;
use super::MemoryLockError;

/// Intentionally a no-op on Windows for VsTerm.
///
/// Upstream `russh` 0.50 routes *every* SSH channel payload (including bulk
/// SFTP `CHANNEL_DATA`) through `CryptoVec`, which calls `VirtualLock` on
/// each allocation. During a large download that is thousands of locks on
/// ~packet-sized buffers. Windows keeps a deliberately tiny lock quota;
/// successful locks pin pages into the process working set and force other
/// pages (egui/wgpu UI) out to the pagefile. After the first big transfer the
/// UI stays laggy (menus, terminal typing) even though the buffers were freed.
///
/// Upstream fixed this properly in later russh by using `Bytes` for non-secret
/// channel data and reserving `CryptoVec`+mlock for keys only. Until we can
/// take that upgrade, skipping `VirtualLock` here is the correct Windows
/// trade-off: channel payloads are not secrets, and under load `VirtualLock`
/// already fails with `ERROR_WORKING_SET_QUOTA` for most packets anyway.
pub fn munlock(_ptr: *const u8, _len: usize) -> Result<(), MemoryLockError> {
    Ok(())
}

pub fn mlock(_ptr: *const u8, _len: usize) -> Result<(), MemoryLockError> {
    Ok(())
}

pub fn memset(ptr: *mut u8, value: i32, size: usize) {
    unsafe {
        libc::memset(ptr as *mut c_void, value, size);
    }
}
