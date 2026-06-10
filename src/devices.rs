use windows::core::PCWSTR;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_PropertyW, CM_Locate_DevNodeW, CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS,
};
use windows::Win32::Devices::HumanInterfaceDevice::HidD_GetProductString;
use windows::Win32::Devices::Properties::{DEVPKEY_NAME, DEVPROPTYPE};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::UI::Input::{
    GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICENAME,
    RIM_TYPEMOUSE,
};

use crate::util::{from_wide, wide};

#[derive(Clone)]
pub struct MouseDevice {
    /// Raw Inputのデバイスハンドル値。再接続で変わるため永続化しない。
    pub handle: isize,
    /// デバイスインターフェイスパス。設定のキーとして使う。
    pub path: String,
    /// 表示用の名前。
    pub name: String,
}

pub fn enumerate_mice() -> Vec<MouseDevice> {
    unsafe {
        let mut count = 0u32;
        let size = size_of::<RAWINPUTDEVICELIST>() as u32;
        if GetRawInputDeviceList(None, &mut count, size) != 0 || count == 0 {
            return Vec::new();
        }
        let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
        let n = GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, size);
        if n == u32::MAX {
            return Vec::new();
        }
        list.truncate(n as usize);
        list.iter()
            .filter(|d| d.dwType == RIM_TYPEMOUSE)
            .filter_map(|d| {
                let path = raw_device_name(d.hDevice)?;
                let name = friendly_name(&path);
                Some(MouseDevice {
                    handle: d.hDevice.0 as isize,
                    path,
                    name,
                })
            })
            .collect()
    }
}

unsafe fn raw_device_name(h: HANDLE) -> Option<String> {
    unsafe {
        let mut len = 0u32;
        GetRawInputDeviceInfoW(Some(h), RIDI_DEVICENAME, None, &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let r = GetRawInputDeviceInfoW(
            Some(h),
            RIDI_DEVICENAME,
            Some(buf.as_mut_ptr() as *mut _),
            &mut len,
        );
        if r == u32::MAX || r == 0 {
            return None;
        }
        Some(from_wide(&buf))
    }
}

fn friendly_name(path: &str) -> String {
    hid_product_string(path)
        .or_else(|| devnode_name(path))
        .unwrap_or_else(|| short_name(path))
}

/// HIDデバイスから製品名文字列を取得する(例: "TPPS/2 Elan TrackPoint")。
fn hid_product_string(path: &str) -> Option<String> {
    unsafe {
        let wpath = wide(path);
        let handle = CreateFileW(
            PCWSTR(wpath.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
        .ok()?;
        let mut buf = [0u16; 127];
        let ok = HidD_GetProductString(
            handle,
            buf.as_mut_ptr() as *mut _,
            (buf.len() * 2) as u32,
        );
        let _ = CloseHandle(handle);
        if !ok {
            return None;
        }
        let s = from_wide(&buf).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

/// デバイスマネージャーに表示される名前(DEVPKEY_NAME)を取得する。
/// PS/2接続のTrackPointなどHIDとして開けないデバイス用のフォールバック。
fn devnode_name(path: &str) -> Option<String> {
    // 例: \\?\HID#VID_1234&PID_5678#7&abc#{guid} → HID\VID_1234&PID_5678\7&abc
    let trimmed = path.strip_prefix(r"\\?\")?;
    let parts: Vec<&str> = trimmed.split('#').collect();
    if parts.len() < 3 {
        return None;
    }
    let instance = format!("{}\\{}\\{}", parts[0], parts[1], parts[2]);
    unsafe {
        let winst = wide(&instance);
        let mut devinst = 0u32;
        if CM_Locate_DevNodeW(&mut devinst, PCWSTR(winst.as_ptr()), CM_LOCATE_DEVNODE_NORMAL)
            != CR_SUCCESS
        {
            return None;
        }
        let mut proptype = DEVPROPTYPE::default();
        let mut size = 0u32;
        let _ = CM_Get_DevNode_PropertyW(devinst, &DEVPKEY_NAME, &mut proptype, None, &mut size, 0);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if CM_Get_DevNode_PropertyW(
            devinst,
            &DEVPKEY_NAME,
            &mut proptype,
            Some(buf.as_mut_ptr()),
            &mut size,
            0,
        ) != CR_SUCCESS
        {
            return None;
        }
        let u16s: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let s = from_wide(&u16s).trim().to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

fn short_name(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .split('#')
        .take(2)
        .collect::<Vec<_>>()
        .join("\\")
}
