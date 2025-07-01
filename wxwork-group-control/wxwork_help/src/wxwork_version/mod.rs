pub mod wxwork_base;
pub mod wxwork_4_1_38_6006;

// 重新导出主要类型
pub use wxwork_base::{WxWorkBase};
pub use wxwork_4_1_38_6006::WxWork4_1_38_6006;

// 版本工厂函数
pub fn create_wxwork_instance(version: &str) -> Option<Box<dyn WxWorkBase>> {
    match version {
        "4.1.38.6006" => Some(Box::new(WxWork4_1_38_6006::new())),
        _ => None,
    }
}
