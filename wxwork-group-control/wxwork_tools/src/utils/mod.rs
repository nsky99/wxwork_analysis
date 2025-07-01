// 重新导出WString结构体和相关类型，使它们可以从外部访问
pub mod wstring;
pub mod string;

// 为了方便使用，可以直接重新导出WString结构体
pub use wstring::WString;
pub use string::String;