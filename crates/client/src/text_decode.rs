pub(crate) fn command_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Some(text) = decode_utf16_le(bytes) {
        return text;
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.strip_prefix('\u{feff}').unwrap_or(text).to_string();
    }
    decode_platform_output(bytes)
}

fn decode_utf16_le(bytes: &[u8]) -> Option<String> {
    let has_bom = bytes.starts_with(&[0xff, 0xfe]);
    let bytes = bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes);
    if bytes.len() < 2 || bytes.len() % 2 != 0 {
        return None;
    }
    if !has_bom {
        let nul_odd_bytes = bytes
            .chunks_exact(2)
            .filter(|chunk| chunk[1] == 0 && chunk[0] != 0)
            .count();
        if nul_odd_bytes * 2 < bytes.len() / 2 {
            return None;
        }
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).ok()
}

#[cfg(windows)]
fn decode_platform_output(bytes: &[u8]) -> String {
    unsafe {
        decode_windows_code_page(bytes, GetOEMCP())
            .or_else(|| decode_windows_code_page(bytes, GetACP()))
            .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
    }
}

#[cfg(not(windows))]
fn decode_platform_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(windows)]
fn decode_windows_code_page(bytes: &[u8], code_page: u32) -> Option<String> {
    let len = i32::try_from(bytes.len()).ok()?;
    let wide_len =
        unsafe { MultiByteToWideChar(code_page, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0) };
    if wide_len <= 0 {
        return None;
    }
    let mut wide = vec![0u16; wide_len as usize];
    let written = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            len,
            wide.as_mut_ptr(),
            wide_len,
        )
    };
    if written <= 0 {
        return None;
    }
    wide.truncate(written as usize);
    String::from_utf16(&wide).ok()
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetACP() -> u32;
    fn GetOEMCP() -> u32;
    fn MultiByteToWideChar(
        CodePage: u32,
        dwFlags: u32,
        lpMultiByteStr: *const u8,
        cbMultiByte: i32,
        lpWideCharStr: *mut u16,
        cchWideChar: i32,
    ) -> i32;
}
