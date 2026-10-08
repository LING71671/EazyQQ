use crate::{args::Args, format::print_json};

pub fn control(args: &Args) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[link(name = "user32")]
        extern "system" {
            fn FindWindowW(class: *const u16, title: *const u16) -> *mut c_void;
            fn ShowWindow(window: *mut c_void, command: i32) -> i32;
            fn SetForegroundWindow(window: *mut c_void) -> i32;
            fn PostMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize)
                -> i32;
            fn SetWindowPos(
                window: *mut c_void,
                after: *mut c_void,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                flags: u32,
            ) -> i32;
            fn IsZoomed(window: *mut c_void) -> i32;
        }
        let title: Vec<u16> = "EazyQQ - 个人专属智能助手\0".encode_utf16().collect();
        let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        let operation = args
            .positional
            .first()
            .map(String::as_str)
            .unwrap_or("status");
        if operation == "status" {
            print_json(&serde_json::json!({"running":!window.is_null()}));
            return Ok(());
        }
        if window.is_null() {
            return Err("The EazyQQ desktop window is not running".into());
        }
        unsafe {
            match operation {
                "show" => {
                    ShowWindow(window, 9);
                    SetForegroundWindow(window);
                }
                "hide" => {
                    ShowWindow(window, 0);
                }
                "minimize" => {
                    ShowWindow(window, 6);
                }
                "maximize" => {
                    ShowWindow(window, 3);
                }
                "toggle-maximize" => {
                    ShowWindow(window, if IsZoomed(window) != 0 { 9 } else { 3 });
                }
                "close" => {
                    if PostMessageW(window, 0x0010, 0, 0) == 0 {
                        return Err("Could not request window close".into());
                    }
                }
                "drag" => {
                    if PostMessageW(window, 0x0112, 0xF012, 0) == 0 {
                        return Err("Could not initiate window move".into());
                    }
                }
                "move" => {
                    let x = args
                        .flag("x")
                        .ok_or("Missing --x")?
                        .parse()
                        .map_err(|_| "Invalid --x")?;
                    let y = args
                        .flag("y")
                        .ok_or("Missing --y")?
                        .parse()
                        .map_err(|_| "Invalid --y")?;
                    if SetWindowPos(window, std::ptr::null_mut(), x, y, 0, 0, 0x0001 | 0x0004) == 0
                    {
                        return Err("Could not move the desktop window".into());
                    }
                }
                _ => return Err(
                    "Use window status|show|hide|minimize|maximize|toggle-maximize|move|drag|close"
                        .into(),
                ),
            }
        }
        print_json(&serde_json::json!({"operation":operation,"ok":true}));
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = args;
        Err("Desktop window control requires Windows".into())
    }
}
