use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, DestroyMenu, DestroyWindow, GetCursorPos,
    PostMessageW, SetForegroundWindow, TrackPopupMenu, HICON, ICONINFO, MF_CHECKED, MF_GRAYED,
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
        let mut nid = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            hIcon: create_icon(),
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

/// 32x32のアイコンをコードで生成する(赤丸+白の上下矢印)。
fn create_icon() -> HICON {
    const S: usize = 32;
    let mut pixels = [0u32; S * S]; // 0xAARRGGBB
    for y in 0..S {
        for x in 0..S {
            let fx = x as f64 - 15.5;
            let fy = y as f64 - 15.5;
            if (fx * fx + fy * fy).sqrt() > 14.5 {
                continue;
            }
            let ax = fx.abs();
            let arrow = (y >= 5 && y <= 10 && ax <= (y as f64 - 4.0) * 0.9)
                || (y >= 21 && y <= 26 && ax <= (27.0 - y as f64) * 0.9)
                || (y > 10 && y < 21 && ax <= 1.6);
            pixels[y * S + x] = if arrow { 0xFFFF_FFFF } else { 0xFFD8_2A2A };
        }
    }
    unsafe {
        let hbm_color = CreateBitmap(S as i32, S as i32, 1, 32, Some(pixels.as_ptr() as *const _));
        let mask = [0u8; S * 4]; // モノクロ1bpp 32x32 = 128バイト
        let hbm_mask = CreateBitmap(S as i32, S as i32, 1, 1, Some(mask.as_ptr() as *const _));
        let info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(hbm_color.into());
        let _ = DeleteObject(hbm_mask.into());
        icon
    }
}
