

// 定义版本接口
pub trait WxWorkBase {
    fn init(&mut self) -> Result<(), String>;
    fn refresh_qrcode(&mut self) -> Result<(), String>;
}

// 基础配置结构体，存放共同的成员变量
#[derive(Debug, Clone)]
pub struct WxWorkConfig {
    pub version: String,                // 版本信息
    pub refresh_qrcode_addr: usize,     // 刷新二维码函数地址
    pub module_name: String,            // 模块名称
}
