// TrackPointのミドルボタン・ドラッグをスクロールに変換する常駐ツール。
//
// 仕組み:
// - Raw Input (WM_INPUT) でイベントの発生元デバイスを特定する(ブロックは不可)。
// - 低レベルマウスフック (WH_MOUSE_LL) で入力をブロック・置換する(デバイス特定は不可)。
// - フックでWM_MBUTTONDOWNを受けたら、対応するWM_INPUT(先にキューに入っている)を
//   その場でポンプして発生元デバイスを突き合わせる。
// - 有効なデバイスなら押下を飲み込み、動かずに離されたら本来のクリックを合成、
//   閾値以上動いたらカーソルを凍結してRaw Inputの移動量をホイールdeltaに変換する。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod devices;
mod engine;
mod tray;
mod util;

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEMOUSE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
    MessageBoxW, PeekMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, MB_ICONERROR, MB_OK,
    MSG, MSLLHOOKSTRUCT, PM_REMOVE, WH_MOUSE_LL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY,
    WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_RBUTTONUP, WNDCLASSW,
};

use engine::{Engine, UpAction, MAGIC_EXTRA};
use util::wide;

static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static IN_PUMP: AtomicBool = AtomicBool::new(false);
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

const LLMHF_INJECTED: u32 = 0x0000_0001;
const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
const MOUSE_MOVE_ABSOLUTE_FLAG: u16 = 0x0001;

pub fn engine() -> &'static Mutex<Engine> {
    ENGINE.get().expect("engine not initialized")
}

fn fatal(msg: &str) {
    let text = wide(msg);
    let title = wide("MiddleClick Scroll for TrackPoint エラー");
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        let log = config::path().with_file_name("panic.log");
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(log, &msg);
        fatal(&msg);
    }));
    if let Err(e) = run() {
        fatal(&format!("起動に失敗しました: {e}"));
        std::process::exit(1);
    }
}

fn run() -> windows::core::Result<()> {
    unsafe {
        // 二重起動防止。
        let mutex_name = wide("Local\\middleclick-scroll-instance");
        let _instance_mutex = CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr()))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            fatal("すでに起動しています。");
            return Ok(());
        }

        let cfg = config::load();
        let devs = devices::enumerate_mice();
        let _ = ENGINE.set(Mutex::new(Engine::new(cfg, devs)));

        let _ = TASKBAR_CREATED.set(RegisterWindowMessageW(PCWSTR(
            wide("TaskbarCreated").as_ptr(),
        )));

        let hinstance = GetModuleHandleW(None)?;
        let class_name = wide("middleclick-scroll-window");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(class_name.as_ptr()),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinstance.into()),
            None,
        )?;
        MAIN_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

        // マウス全般のRaw Inputを受け取る(フォーカス不要、デバイス増減通知つき)。
        let rid = RAWINPUTDEVICE {
            usUsagePage: 0x01, // Generic Desktop
            usUsage: 0x02,     // Mouse
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        };
        RegisterRawInputDevices(&[rid], size_of::<RAWINPUTDEVICE>() as u32)?;

        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0)?;

        tray::add_icon(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWindowsHookEx(hook);
        tray::remove_icon(hwnd);
        Ok(())
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_INPUT => {
                handle_raw_input(lparam);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_INPUT_DEVICE_CHANGE => {
                engine()
                    .lock()
                    .unwrap()
                    .refresh_devices(devices::enumerate_mice());
                LRESULT(0)
            }
            tray::WM_TRAY => {
                let event = lparam.0 as u32;
                if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                    tray::show_menu(hwnd);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            m if Some(&m) == TASKBAR_CREATED.get() => {
                // エクスプローラー再起動時にトレイアイコンを復元する。
                tray::add_icon(hwnd);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn handle_raw_input(lparam: LPARAM) {
    unsafe {
        #[repr(C, align(8))]
        struct Buf([u8; 256]);
        let mut buf = Buf([0; 256]);
        let mut size = size_of::<Buf>() as u32;
        let r = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            Some(buf.0.as_mut_ptr() as *mut _),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        );
        if r == u32::MAX || r == 0 {
            return;
        }
        let raw = &*(buf.0.as_ptr() as *const RAWINPUT);
        if raw.header.dwType != RIM_TYPEMOUSE.0 {
            return;
        }
        let mouse = &raw.data.mouse;
        let device = raw.header.hDevice.0 as isize;
        let button_flags = mouse.Anonymous.Anonymous.usButtonFlags;

        // 注入イベント(device == 0)は突き合わせ対象にしない。
        if button_flags & RI_MOUSE_MIDDLE_BUTTON_DOWN != 0 && device != 0 {
            engine().lock().unwrap().push_middle_down(device);
        }

        if mouse.usFlags.0 & MOUSE_MOVE_ABSOLUTE_FLAG == 0 && (mouse.lLastX != 0 || mouse.lLastY != 0)
        {
            let wheel = engine()
                .lock()
                .unwrap()
                .on_raw_move(device, mouse.lLastX, mouse.lLastY);
            if let Some((v, h)) = wheel {
                engine::send_wheel(v, h);
            }
        }
    }
}

/// フック内で保留中のWM_INPUTをポンプし、押下イベントの発生元デバイスを確定させる。
/// WM_INPUTはフックより先にキューへ投函されるため、通常は即座に見つかる。
unsafe fn pump_raw_input() {
    unsafe {
        let hwnd = HWND(MAIN_HWND.load(Ordering::SeqCst) as *mut _);
        if hwnd.0.is_null() {
            return;
        }
        let deadline = Instant::now() + Duration::from_millis(20);
        loop {
            if engine().lock().unwrap().has_pending_down() {
                return;
            }
            let mut msg = MSG::default();
            if PeekMessageW(&mut msg, Some(hwnd), WM_INPUT, WM_INPUT, PM_REMOVE).as_bool() {
                DispatchMessageW(&msg);
                continue;
            }
            if Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if code < 0 {
            return CallNextHookEx(None, code, wparam, lparam);
        }
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let injected_by_us =
            info.flags & LLMHF_INJECTED != 0 && info.dwExtraInfo == MAGIC_EXTRA;
        let msg = wparam.0 as u32;
        match msg {
            WM_MBUTTONDOWN if !injected_by_us => {
                // ポンプ中の再入(ありえるのはmove程度)では判定しない。
                if !IN_PUMP.swap(true, Ordering::SeqCst) {
                    pump_raw_input();
                    IN_PUMP.store(false, Ordering::SeqCst);
                    if engine().lock().unwrap().on_middle_down() {
                        return LRESULT(1);
                    }
                }
            }
            WM_MBUTTONUP if !injected_by_us => {
                let action = engine().lock().unwrap().on_middle_up();
                match action {
                    UpAction::Pass => {}
                    UpAction::Swallow => return LRESULT(1),
                    UpAction::SynthClick => {
                        engine::send_middle_click();
                        return LRESULT(1);
                    }
                }
            }
            WM_MOUSEMOVE => {
                // スクロール候補/スクロール中はカーソルを動かさない。
                if engine().lock().unwrap().is_active() {
                    return LRESULT(1);
                }
            }
            _ => {}
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}
