use std::alloc::{Layout, alloc, dealloc};
use std::slice;

#[unsafe(no_mangle)]
pub extern "C" fn fewfc_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }

    let layout = Layout::array::<u8>(len).expect("valid wasm allocation layout");

    unsafe { alloc(layout) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fewfc_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }

    let layout = Layout::array::<u8>(len).expect("valid wasm deallocation layout");

    unsafe {
        dealloc(ptr, layout);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fewfc_handle_request(ptr: *const u8, len: usize) -> u64 {
    let input = if ptr.is_null() || len == 0 {
        ""
    } else {
        let bytes = unsafe { slice::from_raw_parts(ptr, len) };
        std::str::from_utf8(bytes).unwrap_or("")
    };
    let output = crate::web_api::handle_request_json(input)
        .unwrap_or_else(|error| serde_json::json!({ "error": error }).to_string());

    pack_response(output)
}

fn pack_response(output: String) -> u64 {
    let bytes = output.as_bytes();
    let len = bytes.len();
    let ptr = fewfc_alloc(len);

    if len > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
        }
    }

    ((ptr as u64) << 32) | (len as u64)
}
