
#[repr(C)]
pub struct WString {
     data: WStringData,   // 实际存储数据的地方
     length: usize,       // 存储的数据长度
     capacity: usize,     // 存储数据的容量
}

#[repr(C)]
union WStringData {
    wstr: [u16; 8],     // 如果数据长度小于8，就直接存储在wstr中
    pwstr: *const u16,   // 如果数据长度大于8，就存储在pwstr中
}

#[allow(dead_code)]
impl WString {
    // 创建一个空的WString
    pub fn new() -> Self {
        WString {
            data: WStringData { wstr: [0; 8] },
            length: 0,
            capacity: 8,
        }
    }
    
    /// 从Rust字符串创建WString
    pub fn from_str(s: &str) -> Self {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        Self::from_utf16(&utf16)
    }
    
    // 从UTF-16数组创建WString
    pub fn from_utf16(utf16: &[u16]) -> Self {
        let len = utf16.len();
        
        // 如果长度小于等于8，直接存储在wstr中
        if len <= 8 {
            let mut result = WString {
                data: WStringData { wstr: [0; 8] },
                length: len,
                capacity: 8,
            };
            
            // 复制数据到wstr
            unsafe {
                for i in 0..len {
                    result.data.wstr[i] = utf16[i];
                }
            }
            
            result
        } else {
            // 否则，分配堆内存并存储指针
            let mut heap_data = Vec::with_capacity(len);
            heap_data.extend_from_slice(utf16);
            
            // 确保不会被释放
            let ptr = heap_data.as_ptr();
            std::mem::forget(heap_data);
            
            WString {
                data: WStringData { pwstr: ptr },
                length: len,
                capacity: len,
            }
        }
    }
    
    // 获取UTF-16字符串内容
    pub fn as_utf16(&self) -> &[u16] {
        unsafe {
            if self.length <= 8 {
                &self.data.wstr[0..self.length]
            } else {
                std::slice::from_raw_parts(self.data.pwstr, self.length)
            }
        }
    }
    
    // 转换为Rust字符串（可能会失败，因为不是所有UTF-16序列都是有效的UTF-8）
    pub fn to_string(&self) -> Result<String, std::string::FromUtf16Error> {
        String::from_utf16(self.as_utf16())
    }
    
    // 设置新的字符串内容
    pub fn set_str(&mut self, s: &str) {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        self.set_utf16(&utf16);
    }
    
    // 设置新的UTF-16内容
    pub fn set_utf16(&mut self, utf16: &[u16]) {
        let len = utf16.len();
        
        // 如果当前是堆分配的，需要先释放
        if self.capacity > 8 {
            unsafe {
                let old_vec = Vec::from_raw_parts(
                    self.data.pwstr as *mut u16,
                    self.length,
                    self.capacity
                );
                drop(old_vec);
            }
        }
        
        // 如果新内容可以放入wstr
        if len <= 8 {
            self.data = WStringData { wstr: [0; 8] };
            unsafe {
                for i in 0..len {
                    self.data.wstr[i] = utf16[i];
                }
            }
            self.length = len;
            self.capacity = 8;
        } else {
            // 否则，分配堆内存
            let mut heap_data = Vec::with_capacity(len);
            heap_data.extend_from_slice(utf16);
            
            let ptr = heap_data.as_ptr();
            let cap = heap_data.capacity();
            std::mem::forget(heap_data);
            
            self.data = WStringData { pwstr: ptr };
            self.length = len;
            self.capacity = cap;
        }
    }
    
    // 清空字符串
    pub fn clear(&mut self) {
        // 如果当前是堆分配的，需要释放
        if self.capacity > 8 {
            unsafe {
                let old_vec = Vec::from_raw_parts(
                    self.data.pwstr as *mut u16,
                    self.length,
                    self.capacity
                );
                drop(old_vec);
            }
        }
        
        self.data = WStringData { wstr: [0; 8] };
        self.length = 0;
        self.capacity = 8;
    }
}

// 实现Drop trait以确保堆内存被正确释放
impl Drop for WString {
    fn drop(&mut self) {
        if self.capacity > 8 {
            unsafe {
                let _ = Vec::from_raw_parts(
                    self.data.pwstr as *mut u16,
                    self.length,
                    self.capacity
                );
                // Vec会在这里自动释放内存
            }
        }
    }
}