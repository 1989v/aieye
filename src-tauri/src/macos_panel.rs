//! macOS 전용: 일반 Tauri webview window 를 메뉴바 popover 처럼
//! full-screen 앱 위에도 뜨는 floating panel 로 승격.

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
#[cfg(target_os = "macos")]
use std::ffi::c_void;

// NSWindowLevel — "popup menu" 레벨. full-screen 앱 위.
// https://developer.apple.com/documentation/appkit/nswindow/level
#[cfg(target_os = "macos")]
const NS_POP_UP_MENU_WINDOW_LEVEL: i64 = 101;

// NSWindowCollectionBehavior
// canJoinAllSpaces        = 1 << 0 = 1
// fullScreenAuxiliary     = 1 << 8 = 256
// transient               = 1 << 3 = 8   (Dock/Mission Control 에 안 나옴)
// stationary              = 1 << 4 = 16
#[cfg(target_os = "macos")]
const BEHAVIOR: u64 = 1 | 256 | 8;

/// 주어진 NSWindow 를 메뉴바 popover 처럼 설정.
/// - level: popUp (full-screen 위에 올라옴)
/// - collectionBehavior: canJoinAllSpaces + fullScreenAuxiliary + transient
#[cfg(target_os = "macos")]
pub fn elevate_to_panel(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let window = ns_window as *mut AnyObject;
    unsafe {
        let _: () = msg_send![window, setLevel: NS_POP_UP_MENU_WINDOW_LEVEL];
        let _: () = msg_send![window, setCollectionBehavior: BEHAVIOR];
    }
}

/// NSWindow 자체를 완전히 투명하게 만들어 뒤의 NSVisualEffectView 가 보이게.
/// - opaque: NO
/// - backgroundColor: clearColor
#[cfg(target_os = "macos")]
pub fn make_window_transparent(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let window = ns_window as *mut AnyObject;
    unsafe {
        let _: () = msg_send![window, setOpaque: false];
        let ns_color_class = match objc2::runtime::AnyClass::get(c"NSColor") {
            Some(c) => c,
            None => return,
        };
        let clear_color: *mut AnyObject = msg_send![ns_color_class, clearColor];
        let _: () = msg_send![window, setBackgroundColor: clear_color];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn elevate_to_panel(_ns_window: *mut std::ffi::c_void) {}
