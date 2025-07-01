use std::ffi::{CString, OsStr, c_void};
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::slice;
use winapi::shared::minwindef::{DWORD, HINSTANCE, LPCVOID, LPVOID};
use winapi::um::debugapi::OutputDebugStringW;
use winapi::um::libloaderapi::GetModuleHandleA;
use winapi::um::memoryapi::VirtualQueryEx;
use winapi::um::processthreadsapi::GetCurrentProcess;
use winapi::um::winnt::{
    IMAGE_DOS_HEADER, IMAGE_DOS_SIGNATURE, IMAGE_NT_HEADERS, IMAGE_NT_SIGNATURE,
    IMAGE_SECTION_HEADER, MEM_COMMIT, MEM_PRIVATE, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ,
    PAGE_EXECUTE_READWRITE, PAGE_READONLY, PAGE_READWRITE,
};

/// 调试日志输出函数
fn debug_log(message: &str) {
    unsafe {
        let formatted_message = format!("[VTABLE_DEBUG] {}\n", message);
        let wide_message: Vec<u16> = OsStr::new(&formatted_message)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        OutputDebugStringW(wide_message.as_ptr());
    }
}

// RTTI 相关结构体定义
#[repr(C)]
struct TypeDescriptor {
    vtable: *const c_void, // reference to RTTI's vftable
    spare: *const c_void,  // internal runtime reference
    name: [u8; 1],         // 实际上是变长的 type descriptor name
}

#[repr(C)]
struct RTTICompleteObjectLocator {
    signature: u32, // signature is zero
    offset: u32,    // offset of this vtable in complete class (from top)
    cd_offset: u32, // offset of constructor displacement
    type_descriptor_offset: u32, // reference to type description
                    //class_hierarchy_descriptor_offset: u32, // reference to hierarchy description
}

// 通过rtti查找虚函数
pub fn search_vtf_by_rtti(
    module_name: &str,
    rtti_name: &str,
    offset_vtf_in_complete_class: u32,
    offset_constructor: u32,
) -> Option<*const c_void> {
    unsafe {
        // 获取当前模块句柄进行测试
        let module_handle = GetModuleHandleA(CString::new(module_name).unwrap().as_ptr());
        if module_handle.is_null() {
            return None;
        }

        return find_vtable_by_rtti_name(
            module_handle,
            rtti_name,
            offset_vtf_in_complete_class,
            offset_constructor,
        );
    }
}

pub fn search_object_by_rtti(
    module_name: &str,
    rtti_name: &str,
    offset_vtf_in_complete_class: u32,
    offset_constructor: u32,
) -> Option<*mut c_void> {
    debug_log(&format!("开始搜索对象，RTTI名称: {}", rtti_name));

    debug_log(&format!("开始搜索二维码框架对象，RTTI名称: {}", rtti_name));

    // 1. 首先通过RTTI名称找到虚函数表地址
    match search_vtf_by_rtti(
        module_name,
        rtti_name,
        offset_vtf_in_complete_class,
        offset_constructor,
    ) {
        Some(vtable_addr) => {
            debug_log(&format!("找到虚函数表地址: {:p}", vtable_addr));

            // 2. 在内存中搜索指向该虚函数表的对象实例
            let objects = find_objects_in_all_memory_by_vtable(vtable_addr);

            if !objects.is_empty() {
                debug_log(&format!("找到 {} 个对象实例", objects.len()));

                // 返回第一个找到的对象地址
                // 在实际应用中，可能需要进一步验证对象的有效性
                Some(objects[0] as *mut c_void)
            } else {
                debug_log("未找到对象实例");
                None
            }
        }
        None => {
            debug_log(&format!("未找到RTTI名称对应的虚函数表: {}", rtti_name));
            None
        }
    }
}

/// 通过RTTI名称搜索虚函数表地址
///
/// # 参数
/// * `module_handle` - 模块句柄
/// * `rtti_name` - RTTI类型名称，例如 ".?AVQrcodeFrame@ui@wework@@"
/// * `offset_vtf_in_complete_class` - 虚函数表在完整类中的偏移
/// * `offset_constructor` - 虚函数构造函数的偏移
///
/// # 返回值
/// * `Some(*const c_void)` - 找到的虚函数表地址
/// * `None` - 未找到对应的虚函数表
fn find_vtable_by_rtti_name(
    module_handle: HINSTANCE,
    rtti_name: &str,
    offset_vtf_in_complete_class: u32,
    offset_constructor: u32,
) -> Option<*const c_void> {
    debug_log(&format!(
        "开始搜索虚函数表，RTTI名称: {}, 模块句柄: {:p}",
        rtti_name, module_handle
    ));

    unsafe {
        if module_handle.is_null() {
            debug_log("模块句柄为空，搜索失败");
            return None;
        }

        let base_addr = module_handle as *const u8;
        debug_log(&format!("模块基址: {:p}", base_addr));

        // 解析dos头
        let dos_header = base_addr as *const IMAGE_DOS_HEADER;
        if (*dos_header).e_magic != IMAGE_DOS_SIGNATURE {
            debug_log("DOS头签名验证失败");
            return None;
        }
        debug_log("DOS头验证成功");

        // 解析pe头
        let nt_headers =
            (base_addr.offset((*dos_header).e_lfanew as isize)) as *const IMAGE_NT_HEADERS;
        if (*nt_headers).Signature != IMAGE_NT_SIGNATURE {
            debug_log("PE头签名验证失败");
            return None;
        }
        debug_log("PE头验证成功");

        // 获取节表
        let section_header = (nt_headers as *const u8)
            .offset(mem::size_of::<IMAGE_NT_HEADERS>() as isize)
            as *const IMAGE_SECTION_HEADER;

        let section_count = (*nt_headers).FileHeader.NumberOfSections;
        debug_log(&format!("节数量: {}", section_count));

        // 第一步先找通过rtti_name找到type_desc_addr
        debug_log("开始第一步：在.data节中搜索TypeDescriptor");
        // 遍历所有节，查找.data节（通常包含RTTI信息）
        let mut type_desc_addr: Option<*const c_void> = None;
        for i in 0..section_count {
            let section = section_header.offset(i as isize);
            let section_name =
                std::str::from_utf8_unchecked(slice::from_raw_parts((*section).Name.as_ptr(), 8))
                    .trim_end_matches('\0');

            debug_log(&format!("检查节: {}", section_name));
            if section_name == ".data" {
                let section_start = base_addr.offset((*section).VirtualAddress as isize);
                let section_size = *((*section).Misc.VirtualSize()) as usize;
                debug_log(&format!(
                    ".data节地址: {:p}, 大小: {}",
                    section_start, section_size
                ));

                // 在节中搜索RTTI名称
                type_desc_addr =
                    search_type_desc_in_section(section_start, section_size, rtti_name);
                if type_desc_addr.is_some() {
                    debug_log(&format!(
                        "找到TypeDescriptor地址: {:p}",
                        type_desc_addr.unwrap()
                    ));
                }
            }
        }
        // 没通过rtti_name找到TypeDescriptor直接返回None不用继续找了
        if type_desc_addr.is_none() {
            debug_log("第一步失败：未找到TypeDescriptor");
            return None;
        }

        // 第二步通过type_desc找到RTTI Complete Object Locator
        debug_log("开始第二步：在.rdata节中搜索RTTI Complete Object Locator");
        // 遍历所有节，查找.rdata节
        let mut rtti_complete_object_locator_addr: Option<*const c_void> = None;
        for i in 0..section_count {
            let section = section_header.offset(i as isize);
            let section_name =
                std::str::from_utf8_unchecked(slice::from_raw_parts((*section).Name.as_ptr(), 8))
                    .trim_end_matches('\0');

            if section_name == ".rdata" {
                let section_start = base_addr.offset((*section).VirtualAddress as isize);
                let section_size = *((*section).Misc.VirtualSize()) as usize;
                debug_log(&format!(
                    ".rdata节地址: {:p}, 大小: {}",
                    section_start, section_size
                ));
                rtti_complete_object_locator_addr = search_rtti_col_in_section(
                    section_start,
                    section_size,
                    offset_vtf_in_complete_class,
                    offset_constructor,
                    type_desc_addr,
                );
                if rtti_complete_object_locator_addr.is_some() {
                    debug_log(&format!(
                        "找到RTTI Complete Object Locator地址: {:p}",
                        rtti_complete_object_locator_addr.unwrap()
                    ));
                }
            }
        }
        // 没通过type_desc找到RTTI Complete Object Locator直接返回None 不用继续找了
        if rtti_complete_object_locator_addr.is_none() {
            debug_log("第二步失败：未找到RTTI Complete Object Locator");
            return None;
        }

        // 第三步通过RTTI Complete Object Locator找到虚函数表
        debug_log("开始第三步：搜索虚函数表");
        // 遍历所有节，查找.rdata节
        let mut target_vtf: Option<*const c_void> = None;
        for i in 0..section_count {
            let section = section_header.offset(i as isize);
            let section_name =
                std::str::from_utf8_unchecked(slice::from_raw_parts((*section).Name.as_ptr(), 8))
                    .trim_end_matches('\0');

            if section_name == ".rdata" {
                let section_start = base_addr.offset((*section).VirtualAddress as isize);
                let section_size = *((*section).Misc.VirtualSize()) as usize;
                target_vtf = search_vtf_in_section(
                    section_start,
                    section_size,
                    rtti_complete_object_locator_addr,
                );
                if target_vtf.is_some() {
                    debug_log(&format!("找到虚函数表地址: {:p}", target_vtf.unwrap()));
                } else {
                    debug_log("在当前.rdata节中未找到虚函数表");
                }
            }
        }

        if target_vtf.is_some() {
            debug_log("虚函数表搜索成功完成");
        } else {
            debug_log("第三步失败：未找到虚函数表");
        }
        return target_vtf;
    }
}

/// 在指定节中搜索RTTI信息
fn search_type_desc_in_section(
    section_start: *const u8,
    section_size: usize,
    target_name: &str,
) -> Option<*const c_void> {
    debug_log(&format!("在TypeDescriptor中搜索RTTI名称: {}", target_name));
    unsafe {
        let section_data = slice::from_raw_parts(section_start, section_size);

        // 搜索目标RTTI名称
        let target_bytes = target_name.as_bytes();
        debug_log(&format!("搜索字节长度: {}", target_bytes.len()));

        for i in 0..section_data.len().saturating_sub(target_bytes.len()) {
            if &section_data[i..i + target_bytes.len()] == target_bytes {
                // 找到RTTI名称，现在需要找到对应的TypeDescriptor
                let name_addr = section_start.offset(i as isize);
                debug_log(&format!("找到RTTI名称，地址: {:p}", name_addr));

                // TypeDescriptor的name字段前面是vtable和spare指针
                let type_desc_addr =
                    name_addr.offset(-(mem::offset_of!(TypeDescriptor, name) as isize));
                debug_log(&format!("计算得到TypeDescriptor地址: {:p}", type_desc_addr));

                return Some(type_desc_addr as *const c_void);
            }
        }

        debug_log("在当前节中未找到RTTI名称");
        None
    }
}

/// 在指定节中搜索RTTI Complete Object Locator
fn search_rtti_col_in_section(
    section_start: *const u8,
    section_size: usize,
    offset_vtf_in_complete_class: u32,
    offset_constructor: u32,
    type_desc_addr: Option<*const c_void>,
) -> Option<*const c_void> {
    debug_log("开始搜索RTTI Complete Object Locator");
    if type_desc_addr.is_none() {
        debug_log("TypeDescriptor地址为空，无法继续搜索");
        return None;
    }

    // 构造这个对象用于内存搜索,并将结构体转成字节码
    let rtti_col = RTTICompleteObjectLocator {
        signature: 0,
        offset: offset_vtf_in_complete_class,
        cd_offset: offset_constructor,
        type_descriptor_offset: type_desc_addr.unwrap() as u32,
    };
    debug_log(&format!(
        "构造RTTI Complete Object Locator: signature={}, offset={}, cd_offset={}, type_descriptor_offset={:x}",
        rtti_col.signature, rtti_col.offset, rtti_col.cd_offset, rtti_col.type_descriptor_offset
    ));

    let rtti_col_bytes = unsafe {
        std::slice::from_raw_parts(
            &rtti_col as *const RTTICompleteObjectLocator as *const u8,
            std::mem::size_of::<RTTICompleteObjectLocator>(),
        )
    };
    debug_log(&format!(
        "RTTI Complete Object Locator字节长度: {}",
        rtti_col_bytes.len()
    ));

    // 内存搜索
    unsafe {
        let section_data = slice::from_raw_parts(section_start, section_size);

        for i in 0..section_data.len().saturating_sub(rtti_col_bytes.len()) {
            if &section_data[i..i + rtti_col_bytes.len()] == rtti_col_bytes {
                // 找到RTTI Complete Object Locator
                let rtti_col_addr = section_start.offset(i as isize);
                debug_log(&format!(
                    "找到RTTI Complete Object Locator，地址: {:p}",
                    rtti_col_addr
                ));

                return Some(rtti_col_addr as *const c_void);
            }
        }
    }

    debug_log("未找到匹配的RTTI Complete Object Locator");
    None
}

/// 在指定节中搜索虚函数表
fn search_vtf_in_section(
    section_start: *const u8,
    section_size: usize,
    rtti_complete_object_locator_addr: Option<*const c_void>,
) -> Option<*const c_void> {
    debug_log("开始搜索虚函数表");
    if rtti_complete_object_locator_addr.is_none() {
        debug_log("RTTI Complete Object Locator地址为空，无法搜索虚函数表");
        return None;
    }

    let rtti_col_addr = rtti_complete_object_locator_addr.unwrap() as usize;
    debug_log(&format!(
        "搜索指向RTTI Complete Object Locator的指针: {:p}",
        rtti_col_addr as *const c_void
    ));

    // 搜索指向RTTI Complete Object Locator的指针
    let target_bytes = unsafe {
        std::slice::from_raw_parts(
            &rtti_col_addr as *const usize as *const u8,
            std::mem::size_of::<usize>(),
        )
    };
    debug_log(&format!("搜索字节长度: {}", target_bytes.len()));

    unsafe {
        let section_data = slice::from_raw_parts(section_start, section_size);

        // 搜索指向RTTI Complete Object Locator的指针
        for i in 0..section_data.len().saturating_sub(target_bytes.len()) {
            if &section_data[i..i + target_bytes.len()] == target_bytes {
                // 找到指向RTTI Complete Object Locator的指针
                let ptr_addr = section_start.offset(i as isize);
                debug_log(&format!(
                    "找到指向RTTI Complete Object Locator的指针，地址: {:p}",
                    ptr_addr
                ));

                // 虚函数表通常在这个指针的后面（4字节或8字节）
                let vtable_addr = ptr_addr.offset(std::mem::size_of::<*const c_void>() as isize);
                debug_log(&format!("计算得到虚函数表地址: {:p}", vtable_addr));

                return Some(vtable_addr as *const c_void);
            }
        }
    }

    debug_log("未找到虚函数表");
    return None;
}

/// 在所有内存区域中搜索虚函数表地址对应的对象实例
pub fn find_objects_in_all_memory_by_vtable(vtable_addr: *const c_void) -> Vec<*const c_void> {
    debug_log(&format!(
        "开始在所有内存区域中搜索虚函数表地址: {:p}",
        vtable_addr
    ));
    let mut objects = Vec::new();

    unsafe {
        let process_handle = GetCurrentProcess();
        let mut address = std::ptr::null_mut();
        let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

        let (stack_base, stack_limit) = get_precise_stack_range();
        debug_log(&format!(
            "栈基址: {:p}, 栈结尾: {:p}",
            stack_base, stack_limit
        ));

        // 遍历所有内存区域
        while VirtualQueryEx(
            process_handle,
            address,
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        ) != 0
        {
            // 检查内存区域是否可读且已提交
            if mbi.State == MEM_COMMIT  // 已提交
            && is_readable_memory(mbi.Protect) // 可读
            && mbi.Type == MEM_PRIVATE
            // 私有内存
            {
                // 检查当前内存区域是否与栈范围重叠
                let region_start = mbi.BaseAddress as usize;
                let region_end = region_start + mbi.RegionSize;
                let stack_start = stack_limit as usize;
                let stack_end = stack_base as usize;

                // 如果内存区域与栈范围重叠，跳过搜索
                if region_start < stack_end && region_end > stack_start {
                    debug_log(&format!(
                        "跳过栈内存区域: {:p} - {:p}, 栈范围: {:p} - {:p}",
                        mbi.BaseAddress,
                        (region_end) as *const c_void,
                        stack_limit,
                        stack_base
                    ));
                } else {
                    debug_log(&format!(
                        "搜索内存区域: {:p} - {:p}, 大小: {}, 内存保护: 0x{:x} 初始内存保护: 0x{:x}",
                        mbi.BaseAddress,
                        (mbi.BaseAddress as usize + mbi.RegionSize) as *const c_void,
                        mbi.RegionSize,
                        mbi.Protect,
                        mbi.AllocationProtect,
                    ));

                    // 在此内存区域中搜索虚函数表
                    let found_objects = search_memory_region_for_vtable(
                        mbi.BaseAddress as *const u8,
                        mbi.RegionSize,
                        vtable_addr,
                    );
                    objects.extend(found_objects);
                }
            }

            // 移动到下一个内存区域
            address = (mbi.BaseAddress as usize + mbi.RegionSize) as LPVOID;
        }
    }

    debug_log(&format!(
        "在所有内存区域中找到 {} 个对象实例",
        objects.len()
    ));
    objects
}

/// 使用更精确的方法获取栈范围
fn get_precise_stack_range() -> (*const c_void, *const c_void) {
    unsafe {
        // 方法1：通过 NtQueryInformationThread 获取 TEB
        // 方法2：通过栈指针和内存查询组合

        let stack_var = 0u32;
        let current_sp = &stack_var as *const u32 as usize;

        let mut stack_base: usize = 0;
        let mut stack_limit: usize = 0;
        let mut address = current_sp;
        let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

        // 向上搜索栈基址
        loop {
            if VirtualQueryEx(
                GetCurrentProcess(),
                address as LPCVOID,
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            ) == 0
            {
                break;
            }

            if mbi.State == MEM_COMMIT {
                stack_base = mbi.BaseAddress as usize + mbi.RegionSize;
                address = stack_base;
            } else {
                break;
            }
        }

        // 向下搜索栈限制
        address = current_sp;
        loop {
            if VirtualQueryEx(
                GetCurrentProcess(),
                address as LPCVOID,
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            ) == 0
            {
                break;
            }

            if mbi.State == MEM_COMMIT {
                stack_limit = mbi.BaseAddress as usize;
                if stack_limit == 0 {
                    break;
                }
                address = stack_limit.saturating_sub(1);
            } else {
                break;
            }
        }

        (stack_base as *const c_void, stack_limit as *const c_void)
    }
}

/// 检查内存保护标志是否可读
fn is_readable_memory(protect: DWORD) -> bool {
    match protect {
        PAGE_READONLY | PAGE_READWRITE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE => true,
        _ => false,
    }
}

/// 在指定内存区域中搜索虚函数表地址
fn search_memory_region_for_vtable(
    region_start: *const u8,
    region_size: usize,
    vtable_addr: *const c_void,
) -> Vec<*const c_void> {
    let mut objects = Vec::new();

    unsafe {
        // 将虚函数表地址转换为字节数组进行搜索
        let vtable_bytes = std::slice::from_raw_parts(
            &vtable_addr as *const *const c_void as *const u8,
            std::mem::size_of::<*const c_void>(),
        );

        // 创建内存区域的切片
        let region_data = std::slice::from_raw_parts(region_start, region_size);

        // 按指针大小对齐搜索
        let ptr_size = std::mem::size_of::<*const c_void>();
        let search_end = region_size.saturating_sub(vtable_bytes.len());

        for i in (0..search_end).step_by(ptr_size) {
            if &region_data[i..i + vtable_bytes.len()] == vtable_bytes {
                // 找到匹配的虚函数表指针，这可能是一个对象实例
                let object_addr = region_start.offset(i as isize) as *const c_void;
                debug_log(&format!("找到可能的对象实例: {:p}", object_addr));

                // 验证这是否是一个有效的对象
                if validate_object_at_address(object_addr, vtable_addr) {
                    objects.push(object_addr);
                    debug_log(&format!("确认有效对象实例: {:p}", object_addr));
                }
            }
        }
    }

    objects
}

/// 验证指定地址是否是有效的对象实例
fn validate_object_at_address(object_addr: *const c_void, expected_vtable: *const c_void) -> bool {
    unsafe {
        // 检查地址是否有效
        if object_addr.is_null() {
            return false;
        }

        // 尝试读取对象的虚函数表指针
        let vtable_ptr = *(object_addr as *const *const c_void);

        // 检查虚函数表指针是否匹配
        if vtable_ptr != expected_vtable {
            return false;
        }

        // 可以添加更多验证逻辑，比如检查虚函数表是否指向有效的函数
        // 这里简单验证虚函数表指针不为空
        !vtable_ptr.is_null()
    }
}
