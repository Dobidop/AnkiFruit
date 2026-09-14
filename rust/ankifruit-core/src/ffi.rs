// C ABI consumed by the Swift shell. Kept deliberately tiny: start a backend,
// learn where it is listening and with which token, tear it down.
//
// Ownership rules for the Swift side:
//   - `ankifruit_start` returns an opaque handle, or NULL with `*err_out` set.
//   - Every `char*` returned here must be released with `ankifruit_string_free`.
//   - `ankifruit_stop` consumes the handle; do not touch it afterwards.

use std::ffi::c_char;
use std::ffi::CStr;
use std::ffi::CString;
use std::path::PathBuf;
use std::ptr;

use crate::Instance;

/// Opaque handle. Swift sees this as `OpaquePointer`.
pub struct AnkiFruitHandle {
    instance: Instance,
}

fn into_c_string(s: impl Into<Vec<u8>>) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Starts the backend and its loopback server.
///
/// `preferred_langs` is a comma-separated list such as `"en,ja"`; pass NULL for
/// the default. `web_root` is the directory holding the built web UI, served
/// from the same origin as the API; pass NULL to serve the API only. `port` may
/// be 0 to let the OS assign one. On failure returns NULL and, when `err_out`
/// is non-NULL, stores an owned error string there.
///
/// # Safety
/// `preferred_langs` and `web_root` must each be NULL or a valid NUL-terminated
/// C string. `err_out` must be NULL or a valid, writable `*mut c_char`.
#[no_mangle]
pub unsafe extern "C" fn ankifruit_start(
    preferred_langs: *const c_char,
    web_root: *const c_char,
    port: u16,
    err_out: *mut *mut c_char,
) -> *mut AnkiFruitHandle {
    let langs: Vec<String> = if preferred_langs.is_null() {
        vec!["en".to_string()]
    } else {
        match CStr::from_ptr(preferred_langs).to_str() {
            Ok(s) => s
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => {
                if !err_out.is_null() {
                    *err_out = into_c_string("preferred_langs was not valid UTF-8");
                }
                return ptr::null_mut();
            }
        }
    };

    let root: Option<PathBuf> = if web_root.is_null() {
        None
    } else {
        match CStr::from_ptr(web_root).to_str() {
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => {
                if !err_out.is_null() {
                    *err_out = into_c_string("web_root was not valid UTF-8");
                }
                return ptr::null_mut();
            }
        }
    };

    match Instance::start(&langs, port, false, root) {
        Ok(instance) => Box::into_raw(Box::new(AnkiFruitHandle { instance })),
        Err(err) => {
            if !err_out.is_null() {
                *err_out = into_c_string(format!("{err:#}"));
            }
            ptr::null_mut()
        }
    }
}

/// The loopback port the backend is listening on, or 0 if `handle` is NULL.
///
/// # Safety
/// `handle` must be NULL or a live handle from `ankifruit_start`.
#[no_mangle]
pub unsafe extern "C" fn ankifruit_port(handle: *const AnkiFruitHandle) -> u16 {
    match handle.as_ref() {
        Some(h) => h.instance.port,
        None => 0,
    }
}

/// The bearer token required on every request. Caller owns the result and must
/// release it with `ankifruit_string_free`.
///
/// # Safety
/// `handle` must be NULL or a live handle from `ankifruit_start`.
#[no_mangle]
pub unsafe extern "C" fn ankifruit_token(handle: *const AnkiFruitHandle) -> *mut c_char {
    match handle.as_ref() {
        Some(h) => into_c_string(h.instance.token.clone()),
        None => ptr::null_mut(),
    }
}

/// The linked rslib build hash. Caller owns the result.
#[no_mangle]
pub extern "C" fn ankifruit_anki_buildhash() -> *mut c_char {
    into_c_string(crate::anki_buildhash())
}

/// Shuts the backend down and frees the handle.
///
/// # Safety
/// `handle` must be NULL or a live handle from `ankifruit_start`, and must not
/// be used again after this call.
#[no_mangle]
pub unsafe extern "C" fn ankifruit_stop(handle: *mut AnkiFruitHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Releases a string returned by this library.
///
/// # Safety
/// `s` must be NULL or a pointer previously returned by this library, freed once.
#[no_mangle]
pub unsafe extern "C" fn ankifruit_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}
