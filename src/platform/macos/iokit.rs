//! Minimal IOKit/CoreFoundation FFI used to read IORegistry properties.
//!
//! `PerformanceStatistics` is present on current Apple GPU drivers but is not
//! a cross-version API contract. All access is isolated here and callers must
//! treat a missing property as normal degradation.

use std::ffi::{CString, c_void};
use std::io;

type CfTypeRef = *const c_void;
type CfStringRef = *const c_void;
type CfDictionaryRef = *const c_void;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const libc::c_char) -> *mut c_void;
    fn IOServiceGetMatchingService(main_port: u32, matching: *mut c_void) -> u32;
    fn IORegistryEntryCreateCFProperty(
        entry: u32,
        key: CfStringRef,
        allocator: *const c_void,
        options: u32,
    ) -> CfTypeRef;
    fn IOObjectRelease(object: u32) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        value: *const libc::c_char,
        encoding: u32,
    ) -> CfStringRef;
    fn CFStringGetCString(
        value: CfStringRef,
        buffer: *mut libc::c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> bool;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetValue(dictionary: CfDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFNumberGetValue(number: CfTypeRef, number_type: i32, output: *mut c_void) -> bool;
    fn CFNumberGetTypeID() -> usize;
    fn CFGetTypeID(value: CfTypeRef) -> usize;
    fn CFRelease(value: CfTypeRef);
}

const UTF8: u32 = 0x0800_0100;
const CF_NUMBER_DOUBLE: i32 = 13;

struct CfOwned(CfTypeRef);
impl Drop for CfOwned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

fn cf_string(value: &str) -> io::Result<CfOwned> {
    let value = CString::new(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "CFString contains NUL"))?;
    // SAFETY: CString is NUL terminated and UTF-8 encoded.
    let reference = unsafe { CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), UTF8) };
    if reference.is_null() {
        Err(io::Error::other("CFString allocation failed"))
    } else {
        Ok(CfOwned(reference))
    }
}

pub struct Service(u32);

impl Service {
    pub fn matching(class_name: &str) -> io::Result<Self> {
        let name = CString::new(class_name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "IOKit class contains NUL"))?;
        // SAFETY: class name is NUL terminated. IOKit consumes the matching dictionary.
        let matching = unsafe { IOServiceMatching(name.as_ptr()) };
        if matching.is_null() {
            return Err(io::Error::other("IOServiceMatching failed"));
        }
        // SAFETY: matching is a valid dictionary returned immediately above.
        let service = unsafe { IOServiceGetMatchingService(0, matching) };
        if service == 0 {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("no {class_name} service"),
            ))
        } else {
            Ok(Self(service))
        }
    }

    fn property(&self, property: &str) -> io::Result<CfOwned> {
        let key = cf_string(property)?;
        // SAFETY: service and key remain valid for the call. Create follows the
        // CoreFoundation create rule and is released by CfOwned.
        let value = unsafe { IORegistryEntryCreateCFProperty(self.0, key.0, std::ptr::null(), 0) };
        if value.is_null() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing IORegistry property {property}"),
            ))
        } else {
            Ok(CfOwned(value))
        }
    }

    pub fn string_property(&self, property: &str) -> io::Result<String> {
        let value = self.property(property)?;
        // SAFETY: value is a live CF object.
        if unsafe { CFGetTypeID(value.0) } != unsafe { CFStringGetTypeID() } {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{property} is not a string"),
            ));
        }
        let mut buffer = vec![0i8; 512];
        // SAFETY: the type and writable output capacity were checked.
        if !unsafe { CFStringGetCString(value.0, buffer.as_mut_ptr(), buffer.len() as isize, UTF8) }
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cannot decode {property}"),
            ));
        }
        // SAFETY: successful CFStringGetCString NUL-terminates the buffer.
        Ok(unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned())
    }

    pub fn dictionary_number(&self, property: &str, key_name: &str) -> io::Result<f64> {
        let dictionary = self.property(property)?;
        // SAFETY: dictionary is a live CF object.
        if unsafe { CFGetTypeID(dictionary.0) } != unsafe { CFDictionaryGetTypeID() } {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{property} is not a dictionary"),
            ));
        }
        let key = cf_string(key_name)?;
        // SAFETY: dictionary and key are valid. The returned value is borrowed.
        let number = unsafe { CFDictionaryGetValue(dictionary.0, key.0) };
        if number.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing {key_name}"),
            ));
        }
        // SAFETY: number is a live borrowed CF object while dictionary is held.
        if unsafe { CFGetTypeID(number) } != unsafe { CFNumberGetTypeID() } {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{key_name} is not numeric"),
            ));
        }
        let mut output = 0f64;
        // SAFETY: the type was checked and output points to a writable double.
        if unsafe { CFNumberGetValue(number, CF_NUMBER_DOUBLE, (&mut output as *mut f64).cast()) } {
            Ok(output)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("cannot decode {key_name}"),
            ))
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { IOObjectRelease(self.0) };
        }
    }
}
