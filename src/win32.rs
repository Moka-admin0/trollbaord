use anyhow::{Result, ensure};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW, SHFileOperationW,
};

pub fn move_to_recycle_bin(path: &Path) -> Result<()> {
    let path = std::path::absolute(path)?;
    let from = double_nul_terminated(path.as_os_str());
    let mut operation = SHFILEOPSTRUCTW {
        hwnd: null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: null_mut(),
        lpszProgressTitle: null(),
    };

    let code = unsafe { SHFileOperationW(&mut operation) };
    ensure!(code == 0 && operation.fAnyOperationsAborted == 0, "couldn't move it to the Recycle Bin (code {code})");
    Ok(())
}

pub fn process_image_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }

    let mut buffer = [0_u16; 1024];
    let mut len = buffer.len() as u32;
    let found = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut len) };
    unsafe { CloseHandle(process) };
    (found != 0).then(|| String::from_utf16_lossy(&buffer[..len as usize]))
}

pub fn file_description(path: &str) -> Option<String> {
    let info = VersionInfo::read(path)?;
    let (language, code_page) = info.translation()?;
    ["FileDescription", "ProductName"]
        .into_iter()
        .find_map(|field| info.text(&format!("\\StringFileInfo\\{language:04x}{code_page:04x}\\{field}")))
}

struct VersionInfo(Vec<u8>);

impl VersionInfo {
    fn read(path: &str) -> Option<Self> {
        let path = nul_terminated(path);
        let size = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), null_mut()) };
        if size == 0 {
            return None;
        }
        let mut data = vec![0_u8; size as usize];
        let read = unsafe { GetFileVersionInfoW(path.as_ptr(), 0, size, data.as_mut_ptr().cast()) };
        (read != 0).then_some(Self(data))
    }

    fn translation(&self) -> Option<(u16, u16)> {
        let (pointer, byte_len) = self.query("\\VarFileInfo\\Translation")?;
        if byte_len < 4 {
            return None;
        }
        let words = unsafe { std::slice::from_raw_parts(pointer, 2) };
        Some((words[0], words[1]))
    }

    fn text(&self, key: &str) -> Option<String> {
        let (pointer, char_len) = self.query(key)?;
        let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(pointer, char_len) });
        let text = text.trim_end_matches('\0').trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    fn query(&self, key: &str) -> Option<(*const u16, usize)> {
        let key = nul_terminated(key);
        let mut pointer = null_mut();
        let mut len = 0_u32;
        let found = unsafe { VerQueryValueW(self.0.as_ptr().cast(), key.as_ptr(), &mut pointer, &mut len) };
        (found != 0 && len > 0 && !pointer.is_null()).then_some((pointer.cast_const().cast(), len as usize))
    }
}

fn nul_terminated(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn double_nul_terminated(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain([0, 0]).collect()
}
