use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, DestroyWindow, GetCursorPos, LoadIconW,
    PostMessageW, SetForegroundWindow, TrackPopupMenu, MF_CHECKED, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, MF_UNCHECKED, SW_SHOWNORMAL, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WM_APP, WM_NULL,
};

use crate::util::wide;
use crate::{config, engine};

pub const WM_TRAY: u32 = WM_APP + 1;
const TRAY_ID: u32 = 1;

const CMD_EXIT: usize = 1;
const CMD_OPEN_CONFIG: usize = 2;
const CMD_DEVICE_BASE: usize = 100;

pub fn add_icon(hwnd: HWND) {
    unsafe {
        let hmodule = GetModuleHandleW(None).unwrap();
        let hicon = LoadIconW(Some(HINSTANCE(hmodule.0)), PCWSTR(1usize as *const u16))
            .unwrap();
        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: hicon,
            ..Default::default()
        };
        let tip = wide("MiddleClick Scroll for TrackPoint");
        nid.szTip[..tip.len()].copy_from_slice(&tip);
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

pub fn remove_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn show_menu(hwnd: HWND) {
    // (パス, 表示名, 有効) のスナップショット。メニュー表示中はロックを持たない。
    let items: Vec<(String, String, bool)> = {
        let eng = engine().lock().unwrap();
        let mut items = Vec::new();
        for d in &eng.devices {
            let enabled = eng
                .config
                .devices
                .get(&d.path)
                .map(|c| c.enabled)
                .unwrap_or(false);
            items.push((d.path.clone(), d.name.clone(), enabled));
        }
        items
    };
    // 同名デバイスには連番を付けて区別できるようにする。
    let mut labels: Vec<String> = items.iter().map(|(_, name, _)| name.clone()).collect();
    for i in 0..labels.len() {
        let name = &items[i].1;
        if items.iter().filter(|(_, n, _)| n == name).count() > 1 {
            let nth = items[..=i].iter().filter(|(_, n, _)| n == name).count();
            labels[i] = format!("{} #{}", name, nth);
        }
    }

    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        if items.is_empty() {
            let text = wide("(マウスデバイスが見つかりません)");
            let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(text.as_ptr()));
        }
        for (i, (_, _, enabled)) in items.iter().enumerate() {
            let check = if *enabled { MF_CHECKED } else { MF_UNCHECKED };
            let text = wide(&labels[i]);
            let _ = AppendMenuW(menu, MF_STRING | check, CMD_DEVICE_BASE + i, PCWSTR(text.as_ptr()));
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let text = wide("設定ファイルを開く");
        let _ = AppendMenuW(menu, MF_STRING, CMD_OPEN_CONFIG, PCWSTR(text.as_ptr()));
        let text = wide("終了");
        let _ = AppendMenuW(menu, MF_STRING, CMD_EXIT, PCWSTR(text.as_ptr()));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // トレイメニューの定石: 前面化しないとメニューが閉じなくなる。
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        )
        .0 as usize;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);

        match cmd {
            CMD_EXIT => {
                let _ = DestroyWindow(hwnd);
            }
            CMD_OPEN_CONFIG => {
                // 設定ファイルが未作成なら現在の設定で作る。
                {
                    let eng = engine().lock().unwrap();
                    if !config::path().exists() {
                        config::save(&eng.config);
                    }
                }
                let exe = wide("notepad.exe");
                let arg = wide(&config::path().to_string_lossy());
                ShellExecuteW(
                    None,
                    PCWSTR::null(),
                    PCWSTR(exe.as_ptr()),
                    PCWSTR(arg.as_ptr()),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            }
            c if c >= CMD_DEVICE_BASE && c < CMD_DEVICE_BASE + items.len() => {
                let path = &items[c - CMD_DEVICE_BASE].0;
                engine().lock().unwrap().toggle_device(path);
            }
            _ => {}
        }
    }
}
