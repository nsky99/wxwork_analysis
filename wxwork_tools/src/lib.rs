use std::{ffi::*,  sync::Mutex, *};
use windows::{
    Win32::{
        Foundation::*,
        System::{Console::*, LibraryLoader::*, SystemServices::*,Diagnostics::Debug::*},

    },
    core::*,
};

use retour::GenericDetour;

mod utils;

fn debug_log(message: &str) {
    let formatted_message = format!("[wxwork_log] {}\n", message);
    let c_string = std::ffi::CString::new(formatted_message).unwrap();
    unsafe {
        OutputDebugStringA(PCSTR(c_string.as_ptr() as *const u8));
    }
}

// 要hook的函数指针
type LoadXMLFn =
    extern "cdecl" fn(_this: *const c_void, path: *const u16, flag: i32) -> *const utils::WString;

// 日志
type WriteLogFn = extern "cdecl" fn(_this: *const c_void, log: *const utils::String);

// 保存原始函数的静态变量
static ORIGINAL_LOAD_XML: Mutex<Option<GenericDetour<LoadXMLFn>>> = Mutex::new(None);
static ORIGINAL_WRITE_LOG: Mutex<Option<GenericDetour<WriteLogFn>>> = Mutex::new(None);

// 我们的hook函数实现
extern "cdecl" fn load_xml_proxy(
    _this: *const c_void,
    path: *const u16,
    flag: i32,
) -> *const utils::WString {
    unsafe {
        // 打印被拦截的参数
        if !path.is_null() {
            let path_slice = std::slice::from_raw_parts(path, 1024); // 假设最大长度
            let mut len = 0;
            for i in 0..1024 {
                if path_slice[i] == 0 {
                    len = i;
                    break;
                }
            }

            if len > 0 {
                let path_u16 = &path_slice[0..len];
                if let Ok(path_str) = String::from_utf16(path_u16) {
                    debug_log(&format!("LoadXML被调用: 路径={}, 标志={}", path_str, flag));
                }
            }
        } else {
            debug_log(&format!("LoadXML被调用: 路径=NULL, 标志={}", flag));
        }

        // 调用原始函数
        if let Ok(guard) = ORIGINAL_LOAD_XML.lock() {
            if let Some(original) = guard.as_ref() {
                let result = original.call(_this, path, flag);

                // 保存返回的XML内容到文件
                if !result.is_null() && !path.is_null() {
                    let wstring = &*result;
                    if let Ok(content) = wstring.to_string() {
                        // 获取path参数作为文件路径
                        let path_slice = std::slice::from_raw_parts(path, 1024);
                        let mut len = 0;
                        for i in 0..1024 {
                            if path_slice[i] == 0 {
                                len = i;
                                break;
                            }
                        }

                        if len > 0 {
                            let path_u16 = &path_slice[0..len];
                            if let Ok(original_path) = String::from_utf16(path_u16) {
                                // 在路径前追加wxwork_ui_rs目录
                                let file_path = format!("wxwork_ui_rs/{}", original_path);

                                // 创建目录（如果不存在）
                                if let Some(parent) = std::path::Path::new(&file_path).parent() {
                                    let _ = std::fs::create_dir_all(parent);
                                }

                                // 保存XML内容到文件
                                if let Err(e) = std::fs::write(&file_path, &content) {
                                    debug_log(&format!("保存XML文件失败: {} - {}", file_path, e));
                                } else {
                                    debug_log(&format!("XML内容已保存到: {}", file_path));
                                }
                            }
                        }
                    }
                }

                return result;
            }
        }

        std::ptr::null()
    }
}

extern "cdecl" fn write_log_proxy(_this: *const c_void, log: *const utils::String) {
    unsafe {
        // 打印被拦截的参数
        if !log.is_null() {
            let wstring = &*log;
            if let Ok(content) = wstring.to_string() {
                debug_log(&format!("{}", content));
            }
        } else {
            debug_log("日志=NULL");
        }

        // 调用原始函数
        if let Ok(guard) = ORIGINAL_WRITE_LOG.lock() {
            if let Some(original) = guard.as_ref() {
                original.call(_this, log);
            }
        }
    }
}

fn hook_load_xml() {
    // 准备hook指定模块的导出函数，模块属于duilib.dll,导出函数是"?LoadXML@CResManager@DuiLib@@SA?AV?$basic_string@_WU?$char_traits@_W@std@@V?$allocator@_W@2@@std@@PB_WH@Z",
    //返回类型是std::wstring* DuiLib::CResManager::LoadXML(void*,wchar_t const *, int);
    // 1. 获取duilib.dll的句柄
    let h_dll = match unsafe { LoadLibraryW(w!("duilib.dll")) } {
        Ok(h) => h,
        Err(e) => {
            debug_log(&format!("LoadLibraryW failed: {:?}", e));
            return;
        }
    };

    // 2. 获取导出函数地址
    let addr = match unsafe {
        GetProcAddress(
            h_dll,
            s!(
                "?LoadXML@CResManager@DuiLib@@SA?AV?$basic_string@_WU?$char_traits@_W@std@@V?$allocator@_W@2@@std@@PB_WH@Z"
            ),
        )
    } {
        Some(addr) => addr,
        None => {
            debug_log("GetProcAddress failed");
            return;
        }
    };

    // 3. 将地址转换为函数指针
    let original_fn: LoadXMLFn = unsafe { std::mem::transmute(addr) };

    // 4. 创建hook
    unsafe {
        match GenericDetour::<LoadXMLFn>::new(original_fn, load_xml_proxy) {
            Ok(detour) => {
                // 启用hook
                if let Err(e) = detour.enable() {
                    debug_log(&format!("Failed to enable hook: {:?}", e));
                    return;
                }

                // 保存原始函数
                if let Ok(mut guard) = ORIGINAL_LOAD_XML.lock() {
                    *guard = Some(detour);
                }
                debug_log("Successfully hooked LoadXML function");
            }
            Err(e) => {
                debug_log(&format!("Failed to create hook: {:?}", e));
            }
        }
    }
}

fn hook_write_log() {
    // 1. 获取duilib.dll的句柄
    let h_dll = match unsafe { LoadLibraryW(w!("WxWork.exe")) } {
        Ok(h) => h,
        Err(e) => {
            debug_log(&format!("LoadLibraryW failed: {:?}", e));
            return;
        }
    };

    // 2. 获取导出函数地址
    let addr = { h_dll.0 as usize + 0x33D158 };

    // 3. 将地址转换为函数指针
    let original_fn: WriteLogFn = unsafe { std::mem::transmute(addr) };

    // 4. 创建hook
    unsafe {
        match GenericDetour::<WriteLogFn>::new(original_fn, write_log_proxy) {
            Ok(detour) => {
                // 启用hook
                if let Err(e) = detour.enable() {
                    debug_log(&format!("Failed to enable hook: {:?}", e));
                    return;
                }

                // 保存原始函数
                if let Ok(mut guard) = ORIGINAL_WRITE_LOG.lock() {
                    *guard = Some(detour);
                }
                debug_log("Successfully hooked WriteLog function");
            }
            Err(e) => {
                debug_log(&format!("Failed to create hook: {:?}", e));
            }
        }
    }
}

fn worker_thread() {
    //hook_load_xml();
    hook_write_log();
}

#[unsafe(no_mangle)] // 防止函数名被混淆，确保Windows能识别DllMain
extern "stdcall" fn DllMain(_hinst: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    match reason {
        DLL_PROCESS_ATTACH => {
            // 为当前进程分配控制台
            unsafe {
                // 分配新控制台
                let _ = AllocConsole();

                // 重定向标准输出到控制台
                let stdout = windows::Win32::System::Console::GetStdHandle(
                    windows::Win32::System::Console::STD_OUTPUT_HANDLE,
                );
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, stdout.unwrap());

                // 重定向标准错误到控制台
                let stderr = windows::Win32::System::Console::GetStdHandle(
                    windows::Win32::System::Console::STD_ERROR_HANDLE,
                );
                let _ = SetStdHandle(STD_ERROR_HANDLE, stderr.unwrap());
            }
            thread::spawn(|| {
                worker_thread();
            });
        }
        DLL_PROCESS_DETACH => {
            // 清理hook
            // if let Ok(mut guard) = ORIGINAL_LOAD_XML.lock() {
            //     if let Some(detour) = guard.take() {
            //         let _ = unsafe { detour.disable() };
            //     }
            // }

            // if let Ok(mut guard) = ORIGINAL_WRITE_LOG.lock() {
            //     if let Some(detour) = guard.take() {
            //         let _ = unsafe { detour.disable() };
            //     }
            // }
        }
        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {}
        _ => {}
    }
    return BOOL::from(true);
}
