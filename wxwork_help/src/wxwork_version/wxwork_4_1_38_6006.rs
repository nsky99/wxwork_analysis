use super::wxwork_base::{WxWorkBase, WxWorkConfig};
use crate::utils::find_vtf_by_rtti_name::{
    search_object_by_rtti
};
use std::ffi::{CString, c_void};
use winapi::um::libloaderapi::GetModuleHandleA;

/// 调试日志输出函数
fn debug_log(message: &str) {
    unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use winapi::um::debugapi::OutputDebugStringW;

        let formatted_message = format!("[WxWork4_1_38_6006] {}\n", message);
        let wide_message: Vec<u16> = OsStr::new(&formatted_message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        OutputDebugStringW(wide_message.as_ptr());
    }
}

pub struct WxWork4_1_38_6006 {
    config: WxWorkConfig,
}

impl WxWork4_1_38_6006 {
    pub fn new() -> Self {
        Self {
            config: WxWorkConfig {
                version: "4.1.38.6006".to_string(),
                refresh_qrcode_addr: 0,
                module_name: "WxWork.exe".to_string(),
            },
        }
    }
}

impl WxWorkBase for WxWork4_1_38_6006 {
    fn init(&mut self) -> Result<(), String> {
        let module_base = unsafe {
            GetModuleHandleA(
                CString::new(self.config.module_name.as_str())
                    .unwrap()
                    .as_ptr(),
            )
        } as usize;

        // 刷新二维码函数地址
        self.config.refresh_qrcode_addr = module_base + (0x34239A0 as usize);

        // 日志输出
        debug_log(&format!("初始化 WxWork 版本: {}", self.config.version));
        debug_log(&format!("模块名称: {}", self.config.module_name));
        debug_log(&format!(
            "刷新二维码偏移: 0x{:X}",
            self.config.refresh_qrcode_addr
        ));

        Ok(())
    }

    // 刷新二维码
    fn refresh_qrcode(&mut self) -> Result<(), String> {
        debug_log("开始刷新二维码");

        // 定义一个静态的Qrcode对象指针
        static mut QRCODE_OBJ_PTR: *mut c_void = std::ptr::null_mut();

        // 检测指针是否已经初始化
        let qrcode_obj_ptr = unsafe {
            if QRCODE_OBJ_PTR.is_null() {
                let qrcode_obj = search_object_by_rtti(&self.config.module_name, ".?AVQrcodeFrame@ui@wework@@", 0, 0);
                if qrcode_obj.is_none() {
                    debug_log("错误：未找到二维码框架对象");
                    return Err("未找到二维码框架对象".to_string());
                }
                QRCODE_OBJ_PTR = qrcode_obj.unwrap();
                debug_log(&format!("找到二维码对象地址: {:p}", QRCODE_OBJ_PTR as *const c_void));
            }
            QRCODE_OBJ_PTR
        };

        // 检测二维码对象指针
        if qrcode_obj_ptr.is_null() {
            debug_log("错误：二维码对象指针无效");
            return Err("二维码对象指针无效".to_string());
        }

        debug_log(&format!("使用二维码对象地址: {:p}", qrcode_obj_ptr));

        // 定义刷新二维码函数指针
        type FnReflashQrcode = extern "thiscall" fn(*mut c_void);

        // 转换二维码刷新函数指针
        let refresh_qrcode_fn: FnReflashQrcode = unsafe {
            std::mem::transmute::<usize, FnReflashQrcode>(self.config.refresh_qrcode_addr)
        };

        debug_log(&format!("准备调用刷新函数，地址: 0x{:X}", self.config.refresh_qrcode_addr));

        // 调用刷新二维码
        let result = std::panic::catch_unwind(|| {
            refresh_qrcode_fn(qrcode_obj_ptr);
        });

        match result {
            Ok(_) => {
                debug_log("二维码刷新成功");
                Ok(())
            }
            Err(_) => {
                debug_log("二维码刷新时发生异常");
                Err("调用刷新函数时发生异常".to_string())
            }
        }
    }
}
