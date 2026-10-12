#![windows_subsystem = "windows"]
mod model;

use model::{Config, Reminder};
use std::{
    collections::HashSet,
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{Diagnostics::ToolHelp::*, LibraryLoader::*, Registry::*, Threading::*},
    UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
};

const TRAY: u32 = WM_APP + 1;
const POLL: usize = 1;
const ANIMATE: usize = 2;
const SHOW: u32 = 101;
const HIDE: u32 = 102;
const SETTINGS: u32 = 103;
const RELOAD: u32 = 104;
const EXIT: u32 = 105;
const STARTUP: u32 = 106;
const TEXT: &str = "겜켰으면 스더 설정 저장하세요";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn from_wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}
fn config_path() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("settings.ini")
}
fn load_config() -> Config {
    std::fs::read_to_string(config_path())
        .map(|s| Config::parse(&s))
        .unwrap_or_default()
}
const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "MabiSaveReminder";
unsafe fn startup_enabled() -> bool {
    let mut key = null_mut();
    if RegOpenKeyExW(
        HKEY_CURRENT_USER,
        wide(RUN_KEY).as_ptr(),
        0,
        KEY_QUERY_VALUE,
        &mut key,
    ) != ERROR_SUCCESS
    {
        return false;
    }
    let mut len = 0;
    let status = RegQueryValueExW(
        key,
        wide(RUN_NAME).as_ptr(),
        null(),
        null_mut(),
        null_mut(),
        &mut len,
    );
    RegCloseKey(key);
    status == ERROR_SUCCESS
}
unsafe fn set_startup(enabled: bool) -> bool {
    let mut key = null_mut();
    if RegCreateKeyExW(
        HKEY_CURRENT_USER,
        wide(RUN_KEY).as_ptr(),
        0,
        null(),
        REG_OPTION_NON_VOLATILE,
        KEY_SET_VALUE,
        null(),
        &mut key,
        null_mut(),
    ) != ERROR_SUCCESS
    {
        return false;
    }
    let result = if enabled {
        match std::env::current_exe() {
            Ok(exe) => {
                let command = wide(&format!("\"{}\"", exe.display()));
                RegSetValueExW(
                    key,
                    wide(RUN_NAME).as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr() as *const u8,
                    (command.len() * 2) as u32,
                )
            }
            Err(_) => ERROR_PATH_NOT_FOUND,
        }
    } else {
        RegDeleteValueW(key, wide(RUN_NAME).as_ptr())
    };
    RegCloseKey(key);
    result == ERROR_SUCCESS || (!enabled && result == ERROR_FILE_NOT_FOUND)
}
struct App {
    config: Config,
    reminder: Reminder,
    clock: Instant,
    font: HFONT,
    small_font: HFONT,
    font_height: i32,
    taskbar_created: u32,
}
impl App {
    fn now(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
    }
}

// All access is confined to the Windows message-loop thread.
unsafe fn app_ptr(hwnd: HWND) -> *mut App {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App
}

unsafe fn processes(c: &Config) -> Option<HashSet<u32>> {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }
    let mut entry: PROCESSENTRY32W = zeroed();
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut ids = HashSet::new();
    let mut ok = Process32FirstW(snapshot, &mut entry);
    while ok != 0 {
        let name = from_wide(&entry.szExeFile).to_lowercase();
        if c.names.contains(&name) {
            let matches = if c.path_contains.is_empty() {
                true
            } else {
                let process =
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if process.is_null() {
                    has_game_window(entry.th32ProcessID)
                } else {
                    let mut buf = vec![0u16; 32768];
                    let mut len = buf.len() as u32;
                    let read = QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut len);
                    CloseHandle(process);
                    if read != 0 {
                        String::from_utf16_lossy(&buf[..len as usize])
                            .to_lowercase()
                            .contains(&c.path_contains)
                    } else {
                        has_game_window(entry.th32ProcessID)
                    }
                }
            };
            if matches {
                ids.insert(entry.th32ProcessID);
            }
        }
        ok = Process32NextW(snapshot, &mut entry);
    }
    CloseHandle(snapshot);
    Some(ids)
}

struct WindowSearch {
    pid: u32,
    found: bool,
}
unsafe extern "system" fn find_window(hwnd: HWND, param: LPARAM) -> i32 {
    let search = &mut *(param as *mut WindowSearch);
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if search.pid == pid && IsWindowVisible(hwnd) != 0 {
        let mut title = [0u16; 512];
        GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let title = from_wide(&title).to_lowercase();
        if title.contains("마비노기") || title.contains("mabinogi") {
            search.found = true;
            return 0;
        }
    }
    1
}
unsafe fn has_game_window(pid: u32) -> bool {
    let mut search = WindowSearch { pid, found: false };
    EnumWindows(Some(find_window), &mut search as *mut _ as LPARAM);
    search.found
}
unsafe fn position(hwnd: HWND) {
    let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
    let mut info: MONITORINFO = zeroed();
    info.cbSize = size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        return;
    }
    let mut dx = 96;
    let mut dy = 96;
    GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    let scale = dx as f64 / 96.0;
    let r = info.rcMonitor;
    let width = r.right - r.left;
    let font_height = (width / 18).min((r.bottom - r.top) / 5).max(24);
    let height = font_height * 7 / 4;
    let p = app_ptr(hwnd);
    if (*p).font_height != font_height {
        if !(*p).font.is_null() {
            DeleteObject((*p).font);
        }
        if !(*p).small_font.is_null() {
            DeleteObject((*p).small_font);
        }
        (*p).font = CreateFontW(
            -font_height,
            0,
            0,
            0,
            800,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("맑은 고딕").as_ptr(),
        );
        (*p).small_font = CreateFontW(
            -(font_height / 7).max((16.0 * scale) as i32),
            0,
            0,
            0,
            500,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("맑은 고딕").as_ptr(),
        );
        (*p).font_height = font_height;
    }
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        r.left,
        r.top,
        width,
        height,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
}
unsafe fn activate_overlay(hwnd: HWND) {
    let p = app_ptr(hwnd);
    SetTimer(
        hwnd,
        ANIMATE,
        if (*p).config.blink_ms == 0 {
            1000
        } else {
            (*p).config.blink_ms
        },
        None,
    );
    position(hwnd);
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn show(hwnd: HWND) {
    let p = app_ptr(hwnd);
    (*p).reminder.show((*p).now());
    activate_overlay(hwnd);
}
unsafe fn hide(hwnd: HWND) {
    (*app_ptr(hwnd)).reminder.dismiss();
    KillTimer(hwnd, ANIMATE);
    ShowWindow(hwnd, SW_HIDE);
}
unsafe fn paint(hwnd: HWND) {
    let p = app_ptr(hwnd);
    let mut ps: PAINTSTRUCT = zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    let mut rect: RECT = zeroed();
    GetClientRect(hwnd, &mut rect);
    let elapsed = (*p)
        .reminder
        .started_ms
        .map(|t| (*p).now().saturating_sub(t))
        .unwrap_or(0);
    let phase = (*p).config.blink_ms != 0 && (elapsed / (*p).config.blink_ms as u64) % 2 == 1;
    let (bg, fg) = if phase {
        (0x0035eaff, 0x00202020)
    } else {
        (0x003020ec, 0x00ffffff)
    };
    let brush = CreateSolidBrush(bg);
    FillRect(dc, &rect, brush);
    DeleteObject(brush);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, fg);
    let old = SelectObject(dc, (*p).font);
    let mut title = rect;
    title.top += rect.bottom / 12;
    title.bottom = rect.bottom * 4 / 5;
    let text = wide(TEXT);
    DrawTextW(
        dc,
        text.as_ptr(),
        (text.len() - 1) as i32,
        &mut title,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    SelectObject(dc, (*p).small_font);
    let seconds = (*p).config.timeout_seconds.saturating_sub(elapsed / 1000);
    let hint = wide(&format!(
        "클릭하면 닫힘 · 자동 닫힘 {:02}:{:02}",
        seconds / 60,
        seconds % 60
    ));
    let mut sub = rect;
    sub.top = rect.bottom * 4 / 5;
    sub.bottom -= rect.bottom / 12;
    DrawTextW(
        dc,
        hint.as_ptr(),
        (hint.len() - 1) as i32,
        &mut sub,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    SelectObject(dc, old);
    EndPaint(hwnd, &ps);
}
unsafe fn app_icon(size: i32) -> HICON {
    let icon = LoadImageW(
        GetModuleHandleW(null()),
        1usize as *const u16,
        IMAGE_ICON,
        size,
        size,
        LR_SHARED,
    ) as HICON;
    if icon.is_null() {
        LoadIconW(null_mut(), IDI_INFORMATION)
    } else {
        icon
    }
}
unsafe fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = zeroed();
    n.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    n.uCallbackMessage = TRAY;
    n.hIcon = app_icon(GetSystemMetrics(SM_CXSMICON));
    let tip = wide("스더저장해 · 우클릭 메뉴");
    n.szTip[..tip.len()].copy_from_slice(&tip);
    n
}
unsafe fn menu(hwnd: HWND) {
    let m = CreatePopupMenu();
    for (id, label) in [
        (SHOW, "알림 테스트"),
        (HIDE, "알림 닫기"),
        (SETTINGS, "설정 파일 열기"),
        (RELOAD, "설정 다시 읽기"),
        (EXIT, "종료"),
    ] {
        AppendMenuW(m, MF_STRING, id as usize, wide(label).as_ptr());
    }
    InsertMenuW(
        m,
        4,
        MF_BYPOSITION
            | MF_STRING
            | if startup_enabled() {
                MF_CHECKED
            } else {
                MF_UNCHECKED
            },
        STARTUP as usize,
        wide("Windows 로그인 시 자동 실행").as_ptr(),
    );
    let mut pt: POINT = zeroed();
    GetCursorPos(&mut pt);
    // A foreground owner lets the tray menu dismiss correctly on outside click.
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(
        m,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        0,
        hwnd,
        null(),
    );
    DestroyMenu(m);
    PostMessageW(hwnd, WM_NULL, 0, 0);
    if cmd != 0 {
        PostMessageW(hwnd, WM_COMMAND, cmd as usize, 0);
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let p = app_ptr(hwnd);
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == (*p).taskbar_created {
        Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd));
        return 0;
    }
    match msg {
        WM_TIMER if w == POLL => {
            if let Some(ids) = processes(&(*p).config) {
                let now = (*p).now();
                let was_visible = (*p).reminder.started_ms.is_some();
                if (*p).reminder.observe(ids, now) {
                    activate_overlay(hwnd);
                } else if was_visible && (*p).reminder.started_ms.is_none() {
                    hide(hwnd);
                }
            }
            if (*p).reminder.started_ms.is_some() {
                if (*p)
                    .reminder
                    .expire((*p).now(), (*p).config.timeout_seconds)
                {
                    hide(hwnd);
                } else {
                    position(hwnd);
                }
            }
            0
        }
        WM_TIMER if w == ANIMATE => {
            if (*p)
                .reminder
                .expire((*p).now(), (*p).config.timeout_seconds)
            {
                hide(hwnd);
            } else {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_LBUTTONUP => {
            hide(hwnd);
            0
        }
        WM_CLOSE => {
            hide(hwnd);
            0
        }
        WM_DPICHANGED | WM_DISPLAYCHANGE => {
            if (*p).reminder.started_ms.is_some() {
                position(hwnd);
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        TRAY => {
            match l as u32 {
                WM_RBUTTONUP | WM_CONTEXTMENU => menu(hwnd),
                WM_LBUTTONDBLCLK => show(hwnd),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            match (w & 0xffff) as u32 {
                SHOW => show(hwnd),
                HIDE => hide(hwnd),
                SETTINGS => {
                    ShellExecuteW(
                        null_mut(),
                        wide("open").as_ptr(),
                        wide("notepad.exe").as_ptr(),
                        wide(&format!("\"{}\"", config_path().display())).as_ptr(),
                        null(),
                        SW_SHOWNORMAL,
                    );
                }
                RELOAD => {
                    (*p).config = load_config();
                    if (*p).reminder.started_ms.is_some() {
                        // Retain original deadline when editing settings.
                        SetTimer(
                            hwnd,
                            ANIMATE,
                            if (*p).config.blink_ms == 0 {
                                1000
                            } else {
                                (*p).config.blink_ms
                            },
                            None,
                        );
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
                EXIT => {
                    DestroyWindow(hwnd);
                }
                STARTUP => {
                    if !set_startup(!startup_enabled()) {
                        MessageBoxW(
                            hwnd,
                            wide("자동 실행 설정을 변경하지 못했습니다.").as_ptr(),
                            wide("스더저장해").as_ptr(),
                            MB_ICONERROR,
                        );
                    }
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, POLL);
            KillTimer(hwnd, ANIMATE);
            Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd));
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

fn main() {
    unsafe {
        let preview = std::env::args().any(|a| a == "--test");
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(null(), 0, wide("Local\\MabiSaveReminder_v1").as_ptr());
        if mutex.is_null() {
            return;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return;
        }
        let instance = GetModuleHandleW(null());
        let class = wide("MabiSaveReminderWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_HAND),
            hIcon: app_icon(GetSystemMetrics(SM_CXICON)),
            ..zeroed()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST
                | WS_EX_NOACTIVATE
                | if preview {
                    WS_EX_APPWINDOW
                } else {
                    WS_EX_TOOLWINDOW
                },
            class.as_ptr(),
            wide("스더저장해").as_ptr(),
            WS_POPUP,
            0,
            0,
            940,
            112,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            MessageBoxW(
                null_mut(),
                wide("알림 창을 만들 수 없습니다.").as_ptr(),
                wide("스더저장해").as_ptr(),
                MB_ICONERROR,
            );
            CloseHandle(mutex);
            return;
        }
        SendMessageW(
            hwnd,
            WM_SETICON,
            ICON_SMALL as usize,
            app_icon(GetSystemMetrics(SM_CXSMICON)) as isize,
        );
        let mut app = Box::new(App {
            config: load_config(),
            reminder: Reminder::default(),
            clock: Instant::now(),
            font: null_mut(),
            small_font: null_mut(),
            font_height: 0,
            taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *app as *mut App as isize);
        if Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd)) == 0 {
            MessageBoxW(
                null_mut(),
                wide("트레이 아이콘을 등록하지 못했습니다. 프로그램을 다시 실행하세요.").as_ptr(),
                wide("스더저장해").as_ptr(),
                MB_ICONERROR,
            );
            DestroyWindow(hwnd);
        } else {
            SetTimer(hwnd, POLL, 2000, None);
            PostMessageW(hwnd, WM_TIMER, POLL, 0);
            if preview {
                PostMessageW(hwnd, WM_COMMAND, SHOW as usize, 0);
            }
            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if !app.font.is_null() {
            DeleteObject(app.font);
        }
        if !app.small_font.is_null() {
            DeleteObject(app.small_font);
        }
        CloseHandle(mutex);
    }
}
