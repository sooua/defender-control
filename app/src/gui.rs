use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use windows::core::{BOOL, HSTRING, PCWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DWM_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::Graphics::Gdi::{
    AlphaBlend, BeginPaint, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, Ellipse, EndPaint, FillRect,
    GetStockObject, GetTextExtentPoint32W, InvalidateRect, RoundRect, SelectObject, SetBkColor,
    SetBkMode, SetTextColor, UpdateWindow, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY, DIB_RGB_COLORS,
    DRAW_TEXT_FORMAT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    HBRUSH, HFONT, HGDIOBJ, HPEN, NULL_BRUSH, NULL_PEN, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, ODS_DISABLED, ODS_SELECTED, WM_MOUSELEAVE,
};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, ReleaseCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::defender;
use crate::lang;
use crate::trusted_installer;

// ────────────── Control IDs ──────────────

const IDC_BTN_PRIMARY: u32 = 100; // context toggle: disable/enable
const IDC_BTN_MORE: u32 = 106; // overflow menu

// "More" popup-menu command ids
const MENU_REFRESH: u32 = 10;
const MENU_WATCH: u32 = 11;
const MENU_ENABLE: u32 = 14;
const MENU_DISABLE: u32 = 15;
const MENU_SERVICES: u32 = 16;

const WM_USER_LOG: u32 = WM_USER + 100;
const WM_USER_REFRESH_DONE: u32 = WM_USER + 101;
const WM_USER_OP_DONE: u32 = WM_USER + 102;

const LOG_STEP: usize = 0;
const LOG_SUCCESS: usize = 1;
const LOG_ERROR: usize = 2;

// ────────────── Vercel-style palette ──────────────

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}
// Vercel light system
const C_BG: COLORREF = rgb(255, 255, 255); // page white
const C_PANEL: COLORREF = rgb(255, 255, 255); // card surface
const C_PANEL2: COLORREF = rgb(250, 250, 250); // #fafafa subtle tint
const C_BORDER: COLORREF = rgb(229, 229, 229); // shadow-border ≈ #e5e5e5
const C_TEXT: COLORREF = rgb(23, 23, 23); // #171717
const C_MUTED: COLORREF = rgb(102, 102, 102); // #666
const C_FAINT: COLORREF = rgb(140, 140, 140); // light labels
// Vivid signal colors — used for the status DOT only (decorative, not text).
const C_GREEN: COLORREF = rgb(0, 164, 108); // success dot
const C_RED: COLORREF = rgb(255, 91, 79); // Ship Red dot #ff5b4f
const C_AMBER: COLORREF = rgb(240, 158, 30); // partial dot
// Darker variants for VALUE TEXT so they clear 4.5:1 on white.
const C_GREEN_TX: COLORREF = rgb(8, 120, 78); // ≈ 4.7:1
const C_RED_TX: COLORREF = rgb(200, 30, 40); // ≈ 5.0:1
const C_LOGO: COLORREF = rgb(229, 72, 77); // brand mark red (#e5484d)
const C_DARK: COLORREF = rgb(23, 23, 23); // primary CTA fill
const C_DARK_HOT: COLORREF = rgb(56, 56, 56);
const C_SEC_HOT: COLORREF = rgb(245, 245, 245); // secondary hover
const C_HAIR: COLORREF = rgb(238, 238, 238); // faint row separator

// ────────────── Layout (logical px @96dpi) — flat & minimal ──────────────

const W: i32 = 380;
const TITLEBAR_H: i32 = 44;
const M: i32 = 18;
const INNER_W: i32 = W - 2 * M;

// Status heading (dot + big text) + activity subtitle.
// Rhythm: generous gap after the title bar, tight status→activity→row group,
// then a wide gap before the action row.
const STATUS_Y: i32 = TITLEBAR_H + 20;
const STATUS_H: i32 = 32;
const SUB_Y: i32 = STATUS_Y + STATUS_H + 4;

// Inline status row (实时 / 反病毒 / 篡改)
const ROW_Y: i32 = SUB_Y + 24;
const ROW_H: i32 = 18;

// Buttons
const BTN_Y: i32 = ROW_Y + ROW_H + 26;
const BTN_H: i32 = 40;
const BTN_GAP: i32 = 10;
const MORE_W: i32 = 92;
const PRIMARY_W: i32 = INNER_W - MORE_W - BTN_GAP;

const H: i32 = BTN_Y + BTN_H + M;

// Expandable services panel (shown via More → 服务状态)
const SVC_PANEL_Y: i32 = BTN_Y + BTN_H + 18;
const SVC_HDR_H: i32 = 22;
const SVC_ROW_H: i32 = 24;
const SVC_ROWS: i32 = 6;
const SVC_PANEL_H: i32 = SVC_HDR_H + SVC_ROWS * SVC_ROW_H;
const H_EXPANDED: i32 = SVC_PANEL_Y + SVC_PANEL_H + M;

// Title-bar buttons
const TB_BTN: i32 = 30;
const TB_TOP: i32 = (TITLEBAR_H - TB_BTN) / 2;

// ────────────── Global state ──────────────

static SCALE_X1000: AtomicU32 = AtomicU32::new(1000);
static IS_WATCH_INSTALLED: AtomicBool = AtomicBool::new(false);
static HOT_TITLE: AtomicI32 = AtomicI32::new(0); // 0 none, 1 min, 2 close
static HOT_BTN: AtomicIsize = AtomicIsize::new(0);
static SHOW_SERVICES: AtomicBool = AtomicBool::new(false);

/// Current window height (logical px): taller when the services panel is open.
fn current_h() -> i32 {
    if SHOW_SERVICES.load(Ordering::Relaxed) {
        H_EXPANDED
    } else {
        H
    }
}

struct Snapshot {
    overall: String,
    rt: bool,
    av: bool,
    tamper: bool,
    services: Vec<(String, String, String)>,
}
static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);

// Latest activity/result line shown under the status heading.
static STATUS_LINE: Mutex<String> = Mutex::new(String::new());

struct Theme {
    f_title: HFONT,
    f_big: HFONT,
    f_norm: HFONT,
    f_bold: HFONT,
    f_small: HFONT,
    log_brush: HBRUSH,
}
unsafe impl Send for Theme {}
unsafe impl Sync for Theme {}
static THEME: OnceLock<Theme> = OnceLock::new();

// ────────────── Small helpers ──────────────

fn scale() -> f32 {
    SCALE_X1000.load(Ordering::Relaxed) as f32 / 1000.0
}
fn sc(v: i32) -> i32 {
    (v as f32 * scale()).round() as i32
}
fn sleep_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
unsafe fn hwnd_to_raw(h: HWND) -> isize {
    h.0 as isize
}
unsafe fn raw_to_hwnd(v: isize) -> HWND {
    HWND(v as *mut _)
}

fn post_log(hwnd: HWND, msg: &str, level: usize) {
    let s = Box::into_raw(Box::new(msg.to_string()));
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_USER_LOG, WPARAM(level), LPARAM(s as isize));
    }
}
fn post_op_done(hwnd: HWND, msg: &str, success: bool) {
    let s = Box::into_raw(Box::new(msg.to_string()));
    let level = if success { LOG_SUCCESS } else { LOG_ERROR };
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_USER_OP_DONE, WPARAM(level), LPARAM(s as isize));
    }
}
fn post_refresh_done(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_USER_REFRESH_DONE, WPARAM(0), LPARAM(0));
    }
}

unsafe fn make_font(px: i32, weight: i32) -> HFONT {
    let face = to_wide("Microsoft YaHei UI");
    CreateFontW(
        -sc(px),
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        DEFAULT_QUALITY,
        DEFAULT_PITCH.0 as u32,
        PCWSTR(face.as_ptr()),
    )
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| unsafe {
        Theme {
            f_title: make_font(13, 600),
            f_big: make_font(25, 600), // status hero — stronger hierarchy
            f_norm: make_font(13, 400),
            f_bold: make_font(13, 600),
            f_small: make_font(12, 400), // labels/values — readable, not tiny
            log_brush: CreateSolidBrush(C_PANEL2),
        }
    })
}

// Filled rounded rectangle with optional fill / border.
unsafe fn rrect(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    rad: i32,
    fill: Option<COLORREF>,
    border: Option<COLORREF>,
) {
    let hbrush = match fill {
        Some(c) => CreateSolidBrush(c),
        None => HBRUSH(GetStockObject(NULL_BRUSH).0),
    };
    let hpen = match border {
        Some(c) => CreatePen(PS_SOLID, sc(1).max(1), c),
        None => HPEN(GetStockObject(NULL_PEN).0),
    };
    let ob = SelectObject(hdc, HGDIOBJ(hbrush.0));
    let op = SelectObject(hdc, HGDIOBJ(hpen.0));
    let _ = RoundRect(hdc, l, t, r, b, rad, rad);
    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    if fill.is_some() {
        let _ = DeleteObject(HGDIOBJ(hbrush.0));
    }
    if border.is_some() {
        let _ = DeleteObject(HGDIOBJ(hpen.0));
    }
}

unsafe fn dot(hdc: windows::Win32::Graphics::Gdi::HDC, cx: i32, cy: i32, rad: i32, color: COLORREF) {
    let b = CreateSolidBrush(color);
    let ob = SelectObject(hdc, HGDIOBJ(b.0));
    let op = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = Ellipse(hdc, cx - rad, cy - rad, cx + rad, cy + rad);
    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    let _ = DeleteObject(HGDIOBJ(b.0));
}

// Real vector logos, pre-rasterized to 96×96 alpha masks (build-time via resvg).
const ICON_RES: usize = 96;
static GH_ALPHA: &[u8] = include_bytes!("../icons/github_alpha.bin");
static GLOBE_ALPHA: &[u8] = include_bytes!("../icons/globe_alpha.bin");
static LOGO_ALPHA: &[u8] = include_bytes!("../icons/logo_alpha.bin");

/// Draw an embedded alpha-mask icon, tinted to `ink`, box-averaged down to
/// `box_sz` and alpha-blended onto the DC (crisp at any size, recolorable).
unsafe fn blit_icon(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    x: i32,
    y: i32,
    box_sz: i32,
    src: &[u8],
    ink: COLORREF,
) {
    let bs = box_sz.max(1) as usize;
    let ir = ink.0 & 0xFF;
    let ig = (ink.0 >> 8) & 0xFF;
    let ib = (ink.0 >> 16) & 0xFF;
    let mut buf = vec![0u8; bs * bs * 4];
    for ty in 0..bs {
        let sy0 = ty * ICON_RES / bs;
        let sy1 = (((ty + 1) * ICON_RES / bs).max(sy0 + 1)).min(ICON_RES);
        for tx in 0..bs {
            let sx0 = tx * ICON_RES / bs;
            let sx1 = (((tx + 1) * ICON_RES / bs).max(sx0 + 1)).min(ICON_RES);
            let mut sum = 0u32;
            let mut cnt = 0u32;
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    sum += src[sy * ICON_RES + sx] as u32;
                    cnt += 1;
                }
            }
            let a = sum / cnt.max(1);
            let o = (ty * bs + tx) * 4;
            buf[o] = (ib * a / 255) as u8; // premultiplied BGRA
            buf[o + 1] = (ig * a / 255) as u8;
            buf[o + 2] = (ir * a / 255) as u8;
            buf[o + 3] = a as u8;
        }
    }
    let mut bi = BITMAPINFO::default();
    bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bi.bmiHeader.biWidth = bs as i32;
    bi.bmiHeader.biHeight = -(bs as i32); // top-down
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = 0; // BI_RGB
    let mem = CreateCompatibleDC(Some(hdc));
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    if let Ok(dib) = CreateDIBSection(Some(hdc), &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
        if !bits.is_null() {
            std::ptr::copy_nonoverlapping(buf.as_ptr(), bits as *mut u8, buf.len());
        }
        let oldb = SelectObject(mem, HGDIOBJ(dib.0));
        let bf = BLENDFUNCTION {
            BlendOp: 0,            // AC_SRC_OVER
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: 1,        // AC_SRC_ALPHA (premultiplied)
        };
        let _ = AlphaBlend(hdc, x, y, bs as i32, bs as i32, mem, 0, 0, bs as i32, bs as i32, bf);
        SelectObject(mem, oldb);
        let _ = DeleteObject(HGDIOBJ(dib.0));
    }
    let _ = DeleteDC(mem);
}

unsafe fn text_w(hdc: windows::Win32::Graphics::Gdi::HDC, font: HFONT, text: &str) -> i32 {
    SelectObject(hdc, HGDIOBJ(font.0));
    let buf: Vec<u16> = text.encode_utf16().collect();
    let mut sz = SIZE::default();
    let _ = GetTextExtentPoint32W(hdc, &buf, &mut sz);
    sz.cx
}

#[allow(clippy::too_many_arguments)]
unsafe fn dtext(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    l: i32,
    t: i32,
    r: i32,
    b: i32,
    color: COLORREF,
    font: HFONT,
    flags: DRAW_TEXT_FORMAT,
) {
    if text.is_empty() {
        return;
    }
    SelectObject(hdc, HGDIOBJ(font.0));
    SetTextColor(hdc, color);
    SetBkMode(hdc, TRANSPARENT);
    let mut buf: Vec<u16> = text.encode_utf16().collect();
    let mut rc = RECT { left: l, top: t, right: r, bottom: b };
    DrawTextW(hdc, &mut buf, &mut rc, flags);
}

// ────────────── Window procedure ──────────────

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                create_controls(hwnd);
                LRESULT(0)
            }
            WM_NCCALCSIZE if wparam.0 != 0 => LRESULT(0), // remove the native frame/caption
            WM_GETMINMAXINFO => {
                let mmi = &mut *(lparam.0 as *mut MINMAXINFO);
                mmi.ptMinTrackSize = POINT { x: sc(W), y: sc(current_h()) };
                mmi.ptMaxTrackSize = POINT { x: sc(W), y: sc(current_h()) };
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                paint(hwnd, hdc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
                let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut _);
                SetTextColor(hdc, C_TEXT);
                SetBkColor(hdc, C_PANEL2);
                LRESULT(theme().log_brush.0 as isize)
            }
            WM_DRAWITEM => {
                draw_button(&*(lparam.0 as *const DRAWITEMSTRUCT));
                LRESULT(1)
            }
            WM_LBUTTONDOWN => {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                match title_hit(x, y) {
                    2 => {
                        let _ = DestroyWindow(hwnd);
                    }
                    1 => {
                        let _ = ShowWindow(hwnd, SW_MINIMIZE);
                    }
                    3 => on_pick_language(hwnd),
                    4 => open_github(hwnd),
                    _ => {
                        if y < sc(TITLEBAR_H) {
                            let _ = ReleaseCapture();
                            let _ = SendMessageW(
                                hwnd,
                                WM_NCLBUTTONDOWN,
                                Some(WPARAM(HTCAPTION as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
                let hot = title_hit(x, y);
                if HOT_TITLE.swap(hot, Ordering::Relaxed) != hot {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if HOT_TITLE.swap(0, Ordering::Relaxed) != 0 {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 as u32) & 0xFFFF;
                match id {
                    IDC_BTN_PRIMARY => on_primary(hwnd),
                    IDC_BTN_MORE => on_more_menu(hwnd),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_USER_LOG => {
                let s_ptr = lparam.0 as *mut String;
                if !s_ptr.is_null() {
                    let s = Box::from_raw(s_ptr);
                    append_log(hwnd, &s, wparam.0);
                }
                LRESULT(0)
            }
            WM_USER_REFRESH_DONE => {
                let _ = InvalidateRect(Some(hwnd), None, false);
                enable_buttons(hwnd, true);
                LRESULT(0)
            }
            WM_USER_OP_DONE => {
                let s_ptr = lparam.0 as *mut String;
                if !s_ptr.is_null() {
                    let s = Box::from_raw(s_ptr);
                    append_log(hwnd, &s, wparam.0);
                }
                on_refresh(hwnd);
                enable_buttons(hwnd, true);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Physical left-x of the four title buttons: (github, globe, minimize, close).
fn title_btns() -> (i32, i32, i32, i32) {
    let close_l = sc(W) - sc(8) - sc(TB_BTN);
    let min_l = close_l - sc(4) - sc(TB_BTN);
    let globe_l = min_l - sc(4) - sc(TB_BTN);
    let github_l = globe_l - sc(4) - sc(TB_BTN);
    (github_l, globe_l, min_l, close_l)
}

/// 0 none, 1 minimize, 2 close, 3 language, 4 github — for a client point.
fn title_hit(x: i32, y: i32) -> i32 {
    let top = sc(TB_TOP);
    let bot = top + sc(TB_BTN);
    if y < top || y > bot {
        return 0;
    }
    let (github_l, globe_l, min_l, close_l) = title_btns();
    let w = sc(TB_BTN);
    if x >= close_l && x <= close_l + w {
        2
    } else if x >= min_l && x <= min_l + w {
        1
    } else if x >= globe_l && x <= globe_l + w {
        3
    } else if x >= github_l && x <= github_l + w {
        4
    } else {
        0
    }
}

// ────────────── Painting ──────────────

unsafe fn paint(hwnd: HWND, hdc: windows::Win32::Graphics::Gdi::HDC) {
    let s = lang::get_strings();
    let th = theme();

    // Background
    let bg = CreateSolidBrush(C_BG);
    let full = RECT { left: 0, top: 0, right: sc(W), bottom: sc(current_h()) };
    FillRect(hdc, &full, bg);
    let _ = DeleteObject(HGDIOBJ(bg.0));

    let hot = HOT_TITLE.load(Ordering::Relaxed);
    let (github_l, globe_l, min_l, close_l) = title_btns();
    let tbt = sc(TB_TOP);
    let tbb = tbt + sc(TB_BTN);

    // ── Title bar ── brand logo + name; text stops before the button cluster.
    // Logo matches the right-side icon size and shares their vertical center.
    let logo_box = sc(14);
    let logo_y = (sc(TITLEBAR_H) - logo_box) / 2;
    blit_icon(hdc, sc(M), logo_y, logo_box, LOGO_ALPHA, C_LOGO);
    let title_x = sc(M) + logo_box + sc(8);
    dtext(
        hdc,
        s.app_title,
        title_x,
        0,
        github_l - sc(8),
        sc(TITLEBAR_H),
        C_TEXT,
        th.f_title,
        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
    );

    if hot == 4 {
        rrect(hdc, github_l, tbt, github_l + sc(TB_BTN), tbb, sc(8), Some(C_SEC_HOT), None);
    }
    if hot == 3 {
        rrect(hdc, globe_l, tbt, globe_l + sc(TB_BTN), tbb, sc(8), Some(C_SEC_HOT), None);
    }
    if hot == 1 {
        rrect(hdc, min_l, tbt, min_l + sc(TB_BTN), tbb, sc(8), Some(C_SEC_HOT), None);
    }
    if hot == 2 {
        rrect(hdc, close_l, tbt, close_l + sc(TB_BTN), tbb, sc(8), Some(C_RED), None);
    }
    // Vector logos (GitHub mark + globe), tinted & alpha-blended.
    let icon_box = sc(14);
    let icon_off = (sc(TB_BTN) - icon_box) / 2;
    blit_icon(
        hdc,
        github_l + icon_off,
        tbt + icon_off,
        icon_box,
        GH_ALPHA,
        if hot == 4 { C_TEXT } else { C_MUTED },
    );
    blit_icon(
        hdc,
        globe_l + icon_off,
        tbt + icon_off,
        icon_box,
        GLOBE_ALPHA,
        if hot == 3 { C_TEXT } else { C_MUTED },
    );
    dtext(
        hdc,
        s.btn_min,
        min_l,
        tbt,
        min_l + sc(TB_BTN),
        tbb,
        if hot == 1 { C_TEXT } else { C_MUTED },
        th.f_norm,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
    );
    dtext(
        hdc,
        s.btn_close,
        close_l,
        tbt,
        close_l + sc(TB_BTN),
        tbb,
        if hot == 2 { rgb(255, 255, 255) } else { C_MUTED },
        th.f_norm,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
    );

    // Pull snapshot
    let guard = SNAPSHOT.lock().unwrap();
    let snap = guard.as_ref();

    // ── Status heading (flat: dot + big text) ──
    let (overall_txt, overall_color) = match snap.map(|s| s.overall.as_str()) {
        Some("Disabled") => (s.status_disabled, C_RED),
        Some("Protected") => (s.status_protected, C_GREEN),
        Some("Partial") => (s.status_partial, C_AMBER),
        _ => (s.status_unknown, C_MUTED),
    };
    let cy = sc(STATUS_Y) + sc(STATUS_H) / 2;
    dot(hdc, sc(M) + sc(6), cy, sc(6), overall_color);
    dtext(
        hdc,
        overall_txt,
        sc(M) + sc(24),
        sc(STATUS_Y),
        sc(W - M),
        sc(STATUS_Y + STATUS_H),
        C_TEXT,
        th.f_big,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );

    // ── Activity / result subtitle (only while there is something to say) ──
    let activity = STATUS_LINE.lock().unwrap().clone();
    if !activity.is_empty() {
        dtext(
            hdc,
            &activity,
            sc(M),
            sc(SUB_Y),
            sc(W - M),
            sc(SUB_Y) + sc(18),
            C_MUTED,
            th.f_small,
            DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }

    // ── Inline status row: flow left→right by measured width (i18n-safe) ──
    let metrics = [
        (s.realtime_label, snap.map(|s| s.rt)),
        (s.antivirus_label, snap.map(|s| s.av)),
        (s.tamper_label, snap.map(|s| s.tamper)),
    ];
    let ry0 = sc(ROW_Y);
    let right = sc(W - M);
    let gap = sc(16);
    let mut x = sc(M);
    for (label, val) in metrics.iter() {
        let lw = text_w(hdc, th.f_small, label);
        let (vtxt, vcol) = match val {
            None => (s.status_unknown, C_MUTED),
            Some(true) => (s.status_on, C_GREEN_TX),
            Some(false) => (s.status_off, C_RED_TX),
        };
        let vw = text_w(hdc, th.f_bold, vtxt);
        dtext(
            hdc, label, x, ry0, x + lw, ry0 + sc(ROW_H),
            C_MUTED, th.f_small, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let vx = x + lw + sc(5);
        dtext(
            hdc, vtxt, vx, ry0, right, ry0 + sc(ROW_H),
            vcol, th.f_bold, DT_LEFT | DT_SINGLELINE | DT_NOPREFIX,
        );
        x = vx + vw + gap;
    }

    // ── Expandable services panel (same flat surface as the main UI) ──
    if SHOW_SERVICES.load(Ordering::Relaxed) {
        // top divider
        let div = CreateSolidBrush(C_BORDER);
        let dl = RECT {
            left: sc(M),
            top: sc(SVC_PANEL_Y) - sc(2),
            right: sc(W - M),
            bottom: sc(SVC_PANEL_Y) - sc(2) + 1,
        };
        FillRect(hdc, &dl, div);
        let _ = DeleteObject(HGDIOBJ(div.0));

        dtext(
            hdc,
            s.service_group,
            sc(M),
            sc(SVC_PANEL_Y),
            sc(W - M),
            sc(SVC_PANEL_Y) + sc(SVC_HDR_H),
            C_MUTED,
            th.f_small,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );

        let rows_y = sc(SVC_PANEL_Y) + sc(SVC_HDR_H);
        let col_status = sc(M) + (sc(INNER_W) * 50 / 100);
        let col_start = sc(M) + (sc(INNER_W) * 76 / 100);
        if let Some(snap) = snap {
            for (i, (name, status, startup)) in
                snap.services.iter().take(SVC_ROWS as usize).enumerate()
            {
                let ry = rows_y + i as i32 * sc(SVC_ROW_H);
                if i > 0 {
                    let sep = CreateSolidBrush(C_HAIR);
                    let line = RECT { left: sc(M), top: ry, right: sc(W - M), bottom: ry + 1 };
                    FillRect(hdc, &line, sep);
                    let _ = DeleteObject(HGDIOBJ(sep.0));
                }
                dtext(
                    hdc, name, sc(M), ry, col_status - sc(6), ry + sc(SVC_ROW_H),
                    C_TEXT, th.f_norm,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
                );
                let scol = if status == "Running" { C_GREEN_TX } else { C_MUTED };
                dtext(
                    hdc, status, col_status, ry, col_start - sc(6), ry + sc(SVC_ROW_H),
                    scol, th.f_norm, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
                dtext(
                    hdc, startup, col_start, ry, sc(W - M), ry + sc(SVC_ROW_H),
                    C_MUTED, th.f_norm, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
        }
    }

    let _ = hwnd;
}

unsafe fn draw_button(dis: &DRAWITEMSTRUCT) {
    let s = lang::get_strings();
    let th = theme();
    let id = dis.CtlID;
    let hdc = dis.hDC;
    let rc = dis.rcItem;
    let primary = id == IDC_BTN_PRIMARY;
    let pressed = dis.itemState.0 & ODS_SELECTED.0 != 0;
    let disabled = dis.itemState.0 & ODS_DISABLED.0 != 0;
    let hot = HOT_BTN.load(Ordering::Relaxed) == dis.hwndItem.0 as isize;
    let small = false;

    // clear with window bg
    let bg = CreateSolidBrush(C_BG);
    FillRect(hdc, &rc, bg);
    let _ = DeleteObject(HGDIOBJ(bg.0));

    let label = match id {
        IDC_BTN_PRIMARY => primary_label(),
        IDC_BTN_MORE => s.btn_more,
        _ => "",
    };

    let (fill, border, text_color) = if primary {
        // Dark CTA: #171717 fill, white text
        let f = if disabled {
            rgb(240, 240, 240)
        } else if pressed {
            rgb(10, 10, 10)
        } else if hot {
            C_DARK_HOT
        } else {
            C_DARK
        };
        (
            f,
            if disabled { Some(C_BORDER) } else { None },
            if disabled { C_FAINT } else { rgb(255, 255, 255) },
        )
    } else {
        // Secondary: white fill, shadow-border, dark text
        let f = if pressed {
            rgb(235, 235, 235)
        } else if hot && !disabled {
            C_SEC_HOT
        } else {
            C_PANEL
        };
        (f, Some(C_BORDER), if disabled { C_FAINT } else { C_TEXT })
    };

    rrect(
        hdc,
        rc.left,
        rc.top,
        rc.right,
        rc.bottom,
        if small { sc(6) } else { sc(8) },
        Some(fill),
        border,
    );
    dtext(
        hdc,
        label,
        rc.left,
        rc.top,
        rc.right,
        rc.bottom,
        text_color,
        if small {
            th.f_small
        } else if primary {
            th.f_bold
        } else {
            th.f_norm
        },
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
}

// ────────────── Controls ──────────────

extern "system" fn btn_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    unsafe {
        match msg {
            WM_MOUSEMOVE => {
                if HOT_BTN.swap(hwnd.0 as isize, Ordering::Relaxed) != hwnd.0 as isize {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = TrackMouseEvent(&mut tme);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            WM_MOUSELEAVE => {
                if HOT_BTN.swap(0, Ordering::Relaxed) == hwnd.0 as isize {
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

fn create_controls(hwnd: HWND) {
    unsafe {
        let hinst = GetWindowLongPtrW(hwnd, GWLP_HINSTANCE) as isize as *mut _;
        let hinst = HINSTANCE(hinst);

        let btns = [
            (IDC_BTN_PRIMARY, sc(M), sc(PRIMARY_W)),
            (IDC_BTN_MORE, sc(M) + sc(PRIMARY_W) + sc(BTN_GAP), sc(MORE_W)),
        ];
        for (id, x, w) in btns {
            let h = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                &HSTRING::from("BUTTON"),
                &HSTRING::new(),
                WINDOW_STYLE((WS_CHILD | WS_VISIBLE).0 | BS_OWNERDRAW as u32),
                x,
                sc(BTN_Y),
                w,
                sc(BTN_H),
                Some(hwnd),
                Some(HMENU(id as *mut _)),
                Some(hinst),
                None,
            )
            .unwrap_or_default();
            if !h.is_invalid() {
                let _ = SetWindowSubclass(h, Some(btn_subclass), id as usize, 0);
            }
        }

        // Initialize watch-task state from the system (fixes stale button label).
        IS_WATCH_INSTALLED.store(defender::is_watch_installed(), Ordering::Relaxed);

        on_refresh(hwnd);
    }
}

/// Show the latest step/result on the activity line under the status heading.
fn append_log(hwnd: HWND, text: &str, _level: usize) {
    *STATUS_LINE.lock().unwrap() = text.to_string();
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn enable_buttons(hwnd: HWND, enabled: bool) {
    unsafe {
        // Only the primary toggle is disabled mid-operation; More stays usable.
        if let Ok(btn) = GetDlgItem(Some(hwnd), IDC_BTN_PRIMARY as i32) {
            if !btn.is_invalid() {
                let _ = EnableWindow(btn, enabled);
                let _ = InvalidateRect(Some(btn), None, false);
            }
        }
    }
}

// ────────────── Actions ──────────────

fn map_overall(s: &lang::Strings, overall: &str) -> &'static str {
    match overall {
        "Disabled" => s.status_disabled,
        "Protected" => s.status_protected,
        "Partial" => s.status_partial,
        _ => s.status_unknown,
    }
}

/// The primary toggle acts as "enable" only when Defender is fully disabled.
fn primary_is_enable() -> bool {
    let g = SNAPSHOT.lock().unwrap();
    matches!(g.as_ref().map(|s| s.overall.as_str()), Some("Disabled"))
}
fn primary_label() -> &'static str {
    let s = lang::get_strings();
    if primary_is_enable() {
        s.btn_enable
    } else {
        s.btn_disable
    }
}
fn on_primary(hwnd: HWND) {
    if primary_is_enable() {
        on_enable(hwnd);
    } else {
        on_disable(hwnd);
    }
}

fn on_disable(hwnd: HWND) {
    let s = lang::get_strings();
    let confirmed = unsafe {
        MessageBoxW(
            Some(hwnd),
            &HSTRING::from(s.confirm_disable),
            &HSTRING::from(s.confirm_title),
            MB_YESNO | MB_ICONWARNING,
        ) == IDYES
    };
    if !confirmed {
        return;
    }
    enable_buttons(hwnd, false);
    append_log(hwnd, s.log_ti_acquire, LOG_STEP);
    let hwnd_raw = unsafe { hwnd_to_raw(hwnd) };
    std::thread::spawn(move || {
        let hwnd_copy = unsafe { raw_to_hwnd(hwnd_raw) };
        let result = std::panic::catch_unwind(|| {
            let _guard = trusted_installer::escalate_to_ti()
                .map_err(|e| format!("{}{}", s.msg_ti_fail, e))?;

            post_log(hwnd_copy, s.log_restore_point, LOG_STEP);
            defender::create_restore_point();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_backup, LOG_STEP);
            defender::backup_registry();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_disable_tamper, LOG_STEP);
            defender::disable_tamper_protection();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_kill_smartscreen, LOG_STEP);
            defender::kill_smartscreen();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_stop_services, LOG_STEP);
            for svc in &["WinDefend", "WdNisSvc", "SecurityHealthService", "wscsvc"] {
                defender::stop_service(svc);
                defender::set_service_startup(svc, 4);
            }
            sleep_ms(150);

            post_log(hwnd_copy, s.log_strip_ppl, LOG_STEP);
            defender::strip_service_ppl("WinDefend");
            defender::strip_service_ppl("WdNisSvc");
            defender::strip_service_ppl("SecurityHealthService");
            sleep_ms(150);

            post_log(hwnd_copy, s.log_disable_drivers, LOG_STEP);
            defender::set_driver_startup("WdFilter", 4);
            defender::set_driver_startup("WdNisDrv", 4);
            defender::set_driver_startup("WdBoot", 4);
            defender::delete_wdfilter_altitude();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_write_registry, LOG_STEP);
            defender::set_registry_disable();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_mp_pref, LOG_STEP);
            defender::set_mp_preference_disable();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_set_ifeo, LOG_STEP);
            defender::set_ifeo_debugger();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_install_watch, LOG_STEP);
            defender::install_watch_task();
            IS_WATCH_INSTALLED.store(true, Ordering::Relaxed);

            Result::<(), String>::Ok(())
        });
        let (ok, msg) = match result {
            Ok(Ok(())) => {
                let st = defender::get_status();
                if !st.real_time_protection_on && !st.antivirus_enabled {
                    (true, s.msg_disable_ok.to_string())
                } else {
                    (false, format!("{}{}", s.msg_op_fail, map_overall(&s, &st.overall_status)))
                }
            }
            Ok(Err(e)) => (false, e),
            Err(_) => (false, s.msg_op_panic.to_string()),
        };
        post_op_done(hwnd_copy, &msg, ok);
    });
}

fn on_enable(hwnd: HWND) {
    let s = lang::get_strings();
    enable_buttons(hwnd, false);
    append_log(hwnd, s.log_ti_acquire, LOG_STEP);
    let hwnd_raw = unsafe { hwnd_to_raw(hwnd) };
    std::thread::spawn(move || {
        let hwnd_copy = unsafe { raw_to_hwnd(hwnd_raw) };
        let result = std::panic::catch_unwind(|| {
            let _guard = trusted_installer::escalate_to_ti()
                .map_err(|e| format!("{}{}", s.msg_ti_fail, e))?;

            post_log(hwnd_copy, s.log_restore_altitude, LOG_STEP);
            defender::restore_wdfilter_altitude();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_enable_tamper, LOG_STEP);
            defender::enable_tamper_protection();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_restore_ppl, LOG_STEP);
            defender::restore_service_ppl("WinDefend");
            defender::restore_service_ppl("WdNisSvc");
            defender::restore_service_ppl("SecurityHealthService");
            sleep_ms(150);

            // Prefer restoring the user's original config from backup.
            post_log(hwnd_copy, s.log_restore_backup, LOG_STEP);
            let restored = defender::restore_registry_from_backup();
            if !restored {
                post_log(hwnd_copy, s.log_write_registry, LOG_STEP);
                defender::set_registry_enable();
                defender::set_driver_startup("WdFilter", 0);
                defender::set_driver_startup("WdBoot", 0);
                defender::set_driver_startup("WdNisDrv", 3);
            }
            sleep_ms(150);

            post_log(hwnd_copy, s.log_start_services, LOG_STEP);
            defender::set_service_startup("wscsvc", 2);
            defender::set_service_startup("SecurityHealthService", 2);
            defender::set_service_startup("WinDefend", 2);
            defender::set_service_startup("WdNisSvc", 3);
            for svc in &["wscsvc", "SecurityHealthService", "WinDefend", "WdNisSvc"] {
                defender::start_service(svc);
            }
            sleep_ms(150);

            post_log(hwnd_copy, s.log_mp_pref, LOG_STEP);
            defender::set_mp_preference_enable();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_remove_ifeo, LOG_STEP);
            defender::remove_ifeo_debugger();
            sleep_ms(150);

            post_log(hwnd_copy, s.log_remove_watch, LOG_STEP);
            defender::remove_watch_task();
            IS_WATCH_INSTALLED.store(false, Ordering::Relaxed);

            Result::<(), String>::Ok(())
        });
        let (ok, msg) = match result {
            Ok(Ok(())) => {
                let st = defender::get_status();
                if st.real_time_protection_on || st.antivirus_enabled {
                    (true, s.msg_enable_ok.to_string())
                } else {
                    (false, format!("{}{}", s.msg_op_fail, map_overall(&s, &st.overall_status)))
                }
            }
            Ok(Err(e)) => (false, e),
            Err(_) => (false, s.msg_op_panic.to_string()),
        };
        post_op_done(hwnd_copy, &msg, ok);
    });
}

fn on_refresh(hwnd: HWND) {
    let hwnd_raw = unsafe { hwnd_to_raw(hwnd) };
    std::thread::spawn(move || {
        let hwnd_copy = unsafe { raw_to_hwnd(hwnd_raw) };
        let st = defender::get_status();
        let services = st
            .services
            .iter()
            .map(|x| (x.name.clone(), x.status.clone(), x.startup_type.clone()))
            .collect();
        *SNAPSHOT.lock().unwrap() = Some(Snapshot {
            overall: st.overall_status,
            rt: st.real_time_protection_on,
            av: st.antivirus_enabled,
            tamper: st.tamper_protection_on,
            services,
        });
        post_refresh_done(hwnd_copy);
    });
}

fn on_toggle_watch(hwnd: HWND) {
    let s = lang::get_strings();
    let installed = IS_WATCH_INSTALLED.load(Ordering::Relaxed);
    if installed {
        defender::remove_watch_task();
        IS_WATCH_INSTALLED.store(false, Ordering::Relaxed);
        append_log(hwnd, s.msg_watch_removed, LOG_SUCCESS);
    } else {
        defender::install_watch_task();
        IS_WATCH_INSTALLED.store(true, Ordering::Relaxed);
        append_log(hwnd, s.msg_watch_installed, LOG_SUCCESS);
    }
}

fn open_github(hwnd: HWND) {
    let url = to_wide("https://github.com/sooua/defender-control");
    let op = to_wide("open");
    unsafe {
        let _ = ShellExecuteW(
            Some(hwnd),
            PCWSTR(op.as_ptr()),
            PCWSTR(url.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

fn on_pick_language(hwnd: HWND) {
    unsafe {
        let cur = lang::current_lang();
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };
        let items = [
            (1u32, "简体中文", lang::Lang::Zh),
            (2u32, "English", lang::Lang::En),
            (3u32, "日本語", lang::Lang::Ja),
            (4u32, "한국어", lang::Lang::Ko),
        ];
        for (id, label, l) in items {
            let flags = if l == cur { MF_STRING | MF_CHECKED } else { MF_STRING };
            let _ = AppendMenuW(menu, flags, id as usize, &HSTRING::from(label));
        }
        // Position the menu just under the globe button.
        let (_github, globe_l, _min, _close) = title_btns();
        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        let px = wr.left + globe_l;
        let py = wr.top + sc(TB_TOP) + sc(TB_BTN);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN,
            px,
            py,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        let chosen = match cmd.0 {
            1 => Some(lang::Lang::Zh),
            2 => Some(lang::Lang::En),
            3 => Some(lang::Lang::Ja),
            4 => Some(lang::Lang::Ko),
            _ => None,
        };
        if let Some(l) = chosen {
            if l != cur {
                lang::set_lang(l);
                refresh_lang_ui(hwnd);
            }
        }
    }
}

fn refresh_lang_ui(hwnd: HWND) {
    let s = lang::get_strings();
    unsafe {
        let _ = SetWindowTextW(hwnd, &HSTRING::from(s.app_title));
        let _ = InvalidateRect(Some(hwnd), None, false);
        for id in &[IDC_BTN_PRIMARY, IDC_BTN_MORE] {
            if let Ok(b) = GetDlgItem(Some(hwnd), *id as i32) {
                if !b.is_invalid() {
                    let _ = InvalidateRect(Some(b), None, false);
                }
            }
        }
    }
}

fn on_more_menu(hwnd: HWND) {
    let s = lang::get_strings();
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return,
        };
        // Opposite action of the primary toggle, refresh, watch, log ops.
        if primary_is_enable() {
            let _ = AppendMenuW(menu, MF_STRING, MENU_DISABLE as usize, &HSTRING::from(s.btn_disable));
        } else {
            let _ = AppendMenuW(menu, MF_STRING, MENU_ENABLE as usize, &HSTRING::from(s.btn_enable));
        }
        let _ = AppendMenuW(menu, MF_STRING, MENU_REFRESH as usize, &HSTRING::from(s.btn_refresh));
        let _ = AppendMenuW(menu, MF_STRING, MENU_SERVICES as usize, &HSTRING::from(s.service_group));
        let watch_label = if IS_WATCH_INSTALLED.load(Ordering::Relaxed) {
            s.btn_watch_remove
        } else {
            s.btn_watch_install
        };
        let _ = AppendMenuW(menu, MF_STRING, MENU_WATCH as usize, &HSTRING::from(watch_label));

        let mut wr = RECT::default();
        let _ = GetWindowRect(hwnd, &mut wr);
        let px = wr.left + sc(M) + sc(PRIMARY_W) + sc(BTN_GAP);
        let py = wr.top + sc(BTN_Y) + sc(BTN_H) + sc(4);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN,
            px,
            py,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        match cmd.0 as u32 {
            MENU_ENABLE => on_enable(hwnd),
            MENU_DISABLE => on_disable(hwnd),
            MENU_REFRESH => on_refresh(hwnd),
            MENU_SERVICES => toggle_services(hwnd),
            MENU_WATCH => on_toggle_watch(hwnd),
            _ => {}
        }
    }
}

/// Expand/collapse the in-window services panel (consistent with the main UI).
fn toggle_services(hwnd: HWND) {
    let show = !SHOW_SERVICES.load(Ordering::Relaxed);
    SHOW_SERVICES.store(show, Ordering::Relaxed);
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            sc(W),
            sc(current_h()),
            SWP_NOMOVE | SWP_NOZORDER,
        );
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
}

// ────────────── Entry ──────────────

pub fn run() -> Result<(), String> {
    lang::init(); // load persisted language override
    let s = lang::get_strings();
    unsafe {
        let dpi = GetDpiForSystem();
        SCALE_X1000.store(((dpi as f32 / 96.0) * 1000.0) as u32, Ordering::Relaxed);

        let hinst = GetModuleHandleW(None).map_err(|e| format!("{}: {}", s.msg_op_fail, e))?;
        let class_name = HSTRING::from("DefenderControlClass");

        // App icon (embedded as ICON resource id 1 via app.rc).
        let app_icon =
            LoadIconW(Some(HINSTANCE(hinst.0)), PCWSTR(1 as *const u16)).unwrap_or_default();

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: HINSTANCE(hinst.0),
            hIcon: app_icon,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        if RegisterClassW(&wc as *const WNDCLASSW) == 0 {
            return Err(format!("{}: RegisterClassW failed", s.msg_op_fail));
        }

        // No WS_CAPTION → no native title bar. WS_THICKFRAME keeps the DWM
        // drop-shadow + rounded corners + snap; WM_NCCALCSIZE strips its border.
        let style = WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN;
        let scr_w = GetSystemMetrics(SM_CXSCREEN);
        let scr_h = GetSystemMetrics(SM_CYSCREEN);
        let x = (scr_w - sc(W)) / 2;
        let y = (scr_h - sc(H)) / 2;

        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            &class_name,
            &HSTRING::from(s.app_title),
            style,
            x.max(0),
            y.max(0),
            sc(W),
            sc(H),
            None,
            None,
            Some(HINSTANCE(hinst.0)),
            None,
        )
        .map_err(|e| format!("CreateWindowExW failed: {}", e))?;

        // Light mode caption + rounded corners (Win11).
        let dark = BOOL(0);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
        let pref: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const _ as *const _,
            std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        // Taskbar icon
        if !app_icon.is_invalid() {
            let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(ICON_BIG as usize)), Some(LPARAM(app_icon.0 as isize)));
            let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(ICON_SMALL as usize)), Some(LPARAM(app_icon.0 as isize)));
        }

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Ok(())
    }
}
