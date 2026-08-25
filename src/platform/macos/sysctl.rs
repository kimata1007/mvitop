//! Small, allocation-conscious wrappers around macOS `sysctl`.

use std::ffi::CString;
use std::io;
use std::mem::{MaybeUninit, size_of};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn name(name: &str) -> io::Result<CString> {
    CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "sysctl name contains NUL"))
}

pub fn bytes(name_: &str) -> io::Result<Vec<u8>> {
    let name = name(name_)?;
    let mut len = 0usize;
    // SAFETY: sysctlbyname only writes the requested output size here.
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut value = vec![0u8; len];
    // SAFETY: value owns `len` writable bytes and the name is NUL terminated.
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            value.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    value.truncate(len);
    Ok(value)
}

pub fn string(name: &str) -> io::Result<String> {
    let value = bytes(name)?;
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    Ok(String::from_utf8_lossy(&value[..end]).trim().to_owned())
}

pub fn value<T: Copy>(name_: &str) -> io::Result<T> {
    let name = name(name_)?;
    let mut output = MaybeUninit::<T>::uninit();
    let mut len = size_of::<T>();
    // SAFETY: output is properly aligned and sized for T. Kernel sysctls used with
    // this helper have fixed, documented POD representations.
    if unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            output.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    if len != size_of::<T>() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected {name_} size: {len}"),
        ));
    }
    // SAFETY: sysctl filled all bytes and the length was checked above.
    Ok(unsafe { output.assume_init() })
}

pub fn boot_time() -> io::Result<SystemTime> {
    let time: libc::timeval = value("kern.boottime")?;
    Ok(UNIX_EPOCH
        + Duration::new(
            time.tv_sec.max(0) as u64,
            (time.tv_usec.max(0) as u32) * 1_000,
        ))
}

pub fn load_average() -> io::Result<[f64; 3]> {
    let mut loads = [0f64; 3];
    // SAFETY: getloadavg receives space for exactly three doubles.
    let count = unsafe { libc::getloadavg(loads.as_mut_ptr(), loads.len() as i32) };
    if count == 3 {
        Ok(loads)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Returns argv as reported by `KERN_PROCARGS2`. Permission failures are
/// expected for protected processes and are deliberately returned per PID.
pub fn process_arguments(pid: i32, max_size: usize) -> io::Result<Vec<String>> {
    const CTL_KERN: libc::c_int = 1;
    const KERN_PROCARGS2: libc::c_int = 49;
    let mut mib = [CTL_KERN, KERN_PROCARGS2, pid];
    let mut buffer = vec![0u8; max_size];
    let mut len = buffer.len();
    // SAFETY: MIB and destination buffer are valid for their declared lengths.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            buffer.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(len);
    if buffer.len() < size_of::<i32>() {
        return Ok(Vec::new());
    }
    let argc = i32::from_ne_bytes(buffer[..4].try_into().unwrap()).max(0) as usize;
    let mut cursor = 4;
    while cursor < buffer.len() && buffer[cursor] != 0 {
        cursor += 1;
    }
    while cursor < buffer.len() && buffer[cursor] == 0 {
        cursor += 1;
    }
    let mut args = Vec::with_capacity(argc.min(64));
    while cursor < buffer.len() && args.len() < argc {
        let end = buffer[cursor..]
            .iter()
            .position(|b| *b == 0)
            .map(|n| cursor + n)
            .unwrap_or(buffer.len());
        if end > cursor {
            args.push(String::from_utf8_lossy(&buffer[cursor..end]).into_owned());
        }
        cursor = end.saturating_add(1);
        while cursor < buffer.len() && buffer[cursor] == 0 {
            cursor += 1;
        }
    }
    Ok(args)
}

pub fn c_string(bytes: &[libc::c_char]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let bytes = &bytes[..end];
    // SAFETY: c_char is one byte wide on macOS; this only changes signedness.
    let unsigned = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) };
    String::from_utf8_lossy(unsigned).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fixed_and_string_sysctls() {
        assert!(value::<u64>("hw.memsize").unwrap() > 0);
        assert!(!string("hw.model").unwrap().is_empty());
    }
}
