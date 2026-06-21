#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod defender;
mod trusted_installer;
mod lang;
mod gui;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--watch" {
        lang::init();
        defender::run_watch_mode();
        return;
    }

    if let Err(e) = gui::run() {
        let s = lang::get_strings();
        let msg = format!("{}:\n{}", s.startup_fail, e);
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                &windows::core::HSTRING::from(msg.as_str()),
                &windows::core::HSTRING::from(s.error_title),
                windows::Win32::UI::WindowsAndMessaging::MB_OK
                    | windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
    }
}
