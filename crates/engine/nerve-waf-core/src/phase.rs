use serde::{Deserialize, Serialize};

/// 检测阶段 — 对齐 ModSecurity/Coraza phase 概念，便于扩展 response 链
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectPhase {
    /// CC / Geo / Bot / Intel 等前置策略
    PreAccess,
    /// 请求头
    RequestHeaders,
    /// 请求体 / URI / Args
    RequestBody,
    /// 文件上传
    RequestUpload,
    /// 响应头（预留）
    ResponseHeaders,
    /// 响应体（预留）
    ResponseBody,
    /// Lua / 自定义脚本
    Script,
    /// 审计日志
    Logging,
}

impl InspectPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreAccess => "pre_access",
            Self::RequestHeaders => "request_headers",
            Self::RequestBody => "request_body",
            Self::RequestUpload => "request_upload",
            Self::ResponseHeaders => "response_headers",
            Self::ResponseBody => "response_body",
            Self::Script => "script",
            Self::Logging => "logging",
        }
    }
}
