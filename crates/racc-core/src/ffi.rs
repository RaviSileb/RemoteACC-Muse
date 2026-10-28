//! C-ABI pro GUI/mobilní klienty (Flutter přes Dart FFI, Tauri, JNI/XCFramework).
//!
//! Pravidla: všechny vrácené řetězce alokuje Rust a volající je uvolní
//! přes [`racc_free_string`]. Chyby se vracejí jako JSON `{"error": ...}`.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uchar};

use crate::crypto::Identity;

fn to_cstring(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => CString::new("{\"error\":\"nul byte\"}").unwrap().into_raw(),
    }
}

/// Verze knihovny (statický řetězec, neuvolňovat).
#[no_mangle]
pub extern "C" fn racc_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

/// Vygeneruje novou identitu. Vrací JSON `{"secret_hex":..,"fingerprint":..}`.
#[no_mangle]
pub extern "C" fn racc_identity_generate() -> *mut c_char {
    let id = Identity::generate();
    let json = serde_json::json!({
        "secret_hex": hex::encode(id.secret_bytes()),
        "fingerprint": id.fingerprint(),
    });
    to_cstring(json.to_string())
}

/// Spočítá fingerprint z `secret_hex` (64 hex znaků). Vrací JSON nebo chybu.
///
/// # Safety
/// `secret_hex` musí být platný NUL-ukončený řetězec nebo NULL.
#[no_mangle]
pub unsafe extern "C" fn racc_fingerprint_of(secret_hex: *const c_char) -> *mut c_char {
    if secret_hex.is_null() {
        return to_cstring("{\"error\":\"null pointer\"}".to_string());
    }
    let text = unsafe { CStr::from_ptr(secret_hex) }.to_string_lossy();
    let bytes = match hex::decode(text.as_ref()) {
        Ok(b) if b.len() == 32 => b,
        _ => return to_cstring("{\"error\":\"secret_hex must be 64 hex chars\"}".to_string()),
    };
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let id = Identity::from_secret_bytes(&arr);
    let json = serde_json::json!({ "fingerprint": id.fingerprint() });
    to_cstring(json.to_string())
}

/// Parsuje `racc://` URI. Vrací JSON s poli nebo chybu.
///
/// # Safety
/// `uri` musí být platný NUL-ukončený řetězec nebo NULL.
#[no_mangle]
pub unsafe extern "C" fn racc_uri_parse(uri: *const c_char) -> *mut c_char {
    if uri.is_null() {
        return to_cstring("{\"error\":\"null pointer\"}".to_string());
    }
    let text = unsafe { CStr::from_ptr(uri) }.to_string_lossy();
    match text.parse::<crate::uri::RaccUri>() {
        Ok(u) => {
            let json = serde_json::json!({
                "host": u.host,
                "port": u.port,
                "device_id": u.device_id,
                "fingerprint": u.fingerprint,
            });
            to_cstring(json.to_string())
        }
        Err(e) => to_cstring(serde_json::json!({ "error": e.to_string() }).to_string()),
    }
}

/// Uvolní řetězec vrácený touto knihovnou.
///
/// # Safety
/// `s` musí být ukazatel dříve vrácený touto knihovnou, nebo NULL.
/// Každý ukazatel se smí uvolnit právě jednou.
#[no_mangle]
pub unsafe extern "C" fn racc_free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

/// Vrátí 1, pokud vstup vypadá jako platný `racc://` odkaz, jinak 0.
/// (Lehká validace pro GUI bez alokací.)
///
/// # Safety
/// `ptr` musí ukazovat na platných `len` bajtů, nebo být NULL.
#[no_mangle]
pub unsafe extern "C" fn racc_uri_is_valid(ptr: *const c_uchar, len: usize) -> i32 {
    if ptr.is_null() {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    match std::str::from_utf8(bytes) {
        Ok(s) => match s.parse::<crate::uri::RaccUri>() {
            Ok(_) => 1,
            Err(_) => 0,
        },
        Err(_) => 0,
    }
}
