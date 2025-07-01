use std::os::windows::ffi::OsStrExt;
use std::thread;
use winapi::shared::minwindef::{BOOL, DWORD, HINSTANCE};
use winapi::um::debugapi::OutputDebugStringW;
use winapi::um::winnt::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use winapi::um::winuser::{MB_ICONINFORMATION, MB_OK, MessageBoxW};

mod utils;

mod wxwork_version;
use wxwork_version::create_wxwork_instance;

/// 调试日志输出函数
fn debug_log(message: &str) {
    unsafe {
        let formatted_message = format!("[MAIN_DEBUG] {}\n", message);
        let wide_message: Vec<u16> = std::ffi::OsStr::new(&formatted_message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        OutputDebugStringW(wide_message.as_ptr());
    }
}

// 将字符串转换为宽字符串（UTF-16）
fn to_wide_string(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// 线程函数
fn worker_thread() {
    debug_log("工作线程启动");
    if let Some(mut wxwork) = create_wxwork_instance("4.1.38.6006") {
        // 初始化
        let result = wxwork.init();
        if let Err(e) = result {
            debug_log(&format!("初始化失败: {}", e));
            return;
        }

        // 循环刷新二维码
        loop {
            // 检查是否需要刷新登录二维码
            unsafe {
                let message = to_wide_string("刷新二维码");
                let title = to_wide_string("提示");
                MessageBoxW(
                    std::ptr::null_mut(),
                    message.as_ptr(),
                    title.as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }

            // 刷新二维码
            let result = wxwork.refresh_qrcode();
            if let Err(e) = result {
                debug_log(&format!("刷新二维码失败: {}", e));
                return;
            }
        }
    } else {
        unsafe {
            let message = to_wide_string("不支持此版本");
            let title = to_wide_string("提示");
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}

#[unsafe(no_mangle)] // 防止函数名被混淆，确保Windows能识别DllMain
pub extern "stdcall" fn DllMain(_hinst: HINSTANCE, reason: DWORD, _reserved: *mut ()) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            thread::spawn(|| {
                worker_thread();
            });
        }
        DLL_PROCESS_DETACH => {}
        _ => {}
    }
    1
}
