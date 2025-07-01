#[repr(C)]
pub struct String {
     data: StringData,   // 实际存储数据的地方
     length: usize,       // 存储的数据长度
     capacity: usize,     // 存储数据的容量
}

#[repr(C)]
union StringData {
    wstr: [u8; 16],     // 如果数据长度小于16，就直接存储在wstr中
    pwstr: *const u8,   // 如果数据长度大于16，就存储在pwstr中
}

#[allow(dead_code)]
impl String {
    // 创建一个空的String
    pub fn new() -> Self {
        String {
            data: StringData { wstr: [0; 16] },
            length: 0,
            capacity: 16,
        }
    }
    
    /// 从Rust字符串创建String
    pub fn from_str(s: &str) -> Self {
        let bytes = s.as_bytes();
        Self::from_bytes(bytes)
    }
    
    // 从UTF-8字节数组创建String
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let len = bytes.len();
        
        // 如果长度小于等于16，直接存储在wstr中
        if len <= 16 {
            let mut result = String {
                data: StringData { wstr: [0; 16] },
                length: len,
                capacity: 16,
            };
            
            // 复制数据到wstr
            unsafe {
                for i in 0..len {
                    result.data.wstr[i] = bytes[i];
                }
            }
            
            result
        } else {
            // 否则，分配堆内存并存储指针
            let mut heap_data = Vec::with_capacity(len);
            heap_data.extend_from_slice(bytes);
            
            // 确保不会被释放
            let ptr = heap_data.as_ptr();
            std::mem::forget(heap_data);
            
            String {
                data: StringData { pwstr: ptr },
                length: len,
                capacity: len,
            }
        }
    }
    
    // 从UTF-16数组创建String（转换为UTF-8）
    pub fn from_utf16(utf16: &[u16]) -> Result<Self, std::string::FromUtf16Error> {
        let s = std::string::String::from_utf16(utf16)?;
        Ok(Self::from_str(&s))
    }
    
    // 获取UTF-8字节数组内容
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            if self.length <= 16 {
                &self.data.wstr[0..self.length]
            } else {
                std::slice::from_raw_parts(self.data.pwstr, self.length)
            }
        }
    }
    
    // 转换为Rust字符串（可能会失败，因为不是所有字节序列都是有效的UTF-8）
    pub fn to_string(&self) -> Result<std::string::String, std::str::Utf8Error> {
        let bytes = self.as_bytes();
        let s = std::str::from_utf8(bytes)?;
        Ok(s.to_owned())
    }
    
    // 设置新的字符串内容
    pub fn set_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.set_bytes(bytes);
    }
    
    // 设置新的UTF-8字节内容
    pub fn set_bytes(&mut self, bytes: &[u8]) {
        let len = bytes.len();
        
        // 如果当前是堆分配的，需要先释放
        if self.capacity > 16 {
            unsafe {
                let old_vec = Vec::from_raw_parts(
                    self.data.pwstr as *mut u8,
                    self.length,
                    self.capacity
                );
                drop(old_vec);
            }
        }
        
        // 如果新内容可以放入wstr
        if len <= 16 {
            self.data = StringData { wstr: [0; 16] };
            unsafe {
                for i in 0..len {
                    self.data.wstr[i] = bytes[i];
                }
            }
            self.length = len;
            self.capacity = 16;
        } else {
            // 否则，分配堆内存
            let mut heap_data = Vec::with_capacity(len);
            heap_data.extend_from_slice(bytes);
            
            let ptr = heap_data.as_ptr();
            let cap = heap_data.capacity();
            std::mem::forget(heap_data);
            
            self.data = StringData { pwstr: ptr };
            self.length = len;
            self.capacity = cap;
        }
    }
    
    // 清空字符串
    pub fn clear(&mut self) {
        // 如果当前是堆分配的，需要释放
        if self.capacity > 16 {
            unsafe {
                let old_vec = Vec::from_raw_parts(
                    self.data.pwstr as *mut u8,
                    self.length,
                    self.capacity
                );
                drop(old_vec);
            }
        }
        
        self.data = StringData { wstr: [0; 16] };
        self.length = 0;
        self.capacity = 16;
    }
}

// 实现Drop trait以确保堆内存被正确释放
impl Drop for String {
    fn drop(&mut self) {
        if self.capacity > 16 {
            unsafe {
                let _ = Vec::from_raw_parts(
                    self.data.pwstr as *mut u8,
                    self.length,
                    self.capacity
                );
                // Vec会在这里自动释放内存
            }
        }
    }
}