pub use nerve_waf_normalize::{DecodePolicy, DetectionScope};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_true() -> bool {
    true
}

fn default_max_slices() -> usize {
    128
}

fn default_sample_rate() -> u8 {
    100
}

fn default_timeout_ms() -> u32 {
    50
}

/// 规范流水线阶段 id（与 Console `pipeline-stages.ts` 一致）。
/// 语义是增强配置，不是可排序检测段。
pub const PIPELINE_STAGE_IDS: &[&str] = &["pre_access", "expert", "core", "custom"];

fn default_pipeline_order() -> Vec<String> {
    PIPELINE_STAGE_IDS.iter().map(|s| (*s).into()).collect()
}

/// 将历史 `crs` 展开为 `expert` + `core`；剥离已废弃的 `semantic`/`plugin` 段；补齐缺失阶段。
pub fn normalize_pipeline_order(order: &[String]) -> Vec<String> {
    let allowed: std::collections::HashSet<&str> = PIPELINE_STAGE_IDS.iter().copied().collect();
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for id in order {
        if id == "plugin" || id == "semantic" {
            continue;
        }
        if id == "crs" {
            for mapped in ["expert", "core"] {
                if seen.insert(mapped) {
                    result.push(mapped.into());
                }
            }
            continue;
        }
        if allowed.contains(id.as_str()) && seen.insert(id.as_str()) {
            result.push(id.clone());
        }
    }
    for id in PIPELINE_STAGE_IDS {
        if seen.contains(*id) {
            continue;
        }
        seen.insert(*id);
        let canon_idx = PIPELINE_STAGE_IDS.iter().position(|s| *s == *id).unwrap_or(0);
        let mut insert_at = result.len();
        for (i, existing) in result.iter().enumerate() {
            let existing_idx = PIPELINE_STAGE_IDS
                .iter()
                .position(|s| *s == existing.as_str())
                .unwrap_or(usize::MAX);
            if existing_idx > canon_idx {
                insert_at = i;
                break;
            }
        }
        result.insert(insert_at, (*id).into());
    }
    result
}

/// 迁移 `pipeline_enabled`：旧键 `crs` 复制到 expert/core。
pub fn normalize_pipeline_enabled(
    order: &[String],
    enabled: &std::collections::HashMap<String, bool>,
) -> std::collections::HashMap<String, bool> {
    let mut out = std::collections::HashMap::new();
    let crs = enabled.get("crs").copied();
    for id in order {
        let v = enabled
            .get(id)
            .copied()
            .or_else(|| {
                if id == "expert" || id == "core" {
                    crs
                } else {
                    None
                }
            })
            .unwrap_or(true);
        out.insert(id.clone(), v);
    }
    out
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticMode {
    #[default]
    Standard,
    Strict,
    Relaxed,
}

impl SemanticMode {
    pub fn libinjection_base_score(&self) -> u8 {
        match self {
            Self::Strict => 72,
            Self::Standard => 80,
            Self::Relaxed => 88,
        }
    }

    pub fn intent_min_score(&self) -> u8 {
        match self {
            Self::Strict => 55,
            Self::Standard => 65,
            Self::Relaxed => 75,
        }
    }

    pub fn require_suspicious_on_path(&self) -> bool {
        matches!(self, Self::Standard | Self::Relaxed)
    }
}

fn default_fp_threshold() -> u32 {
    3
}

fn default_relax_fp_count() -> u32 {
    10
}

fn default_window_hours() -> u32 {
    168
}

/// 反馈驱动自动调优（长期运营）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticAutoTune {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 同一 param_path 误报达到此次数 → 自动加入参数白名单
    #[serde(default = "default_fp_threshold")]
    pub false_positive_threshold: u32,
    #[serde(default = "default_true")]
    pub auto_whitelist_param: bool,
    /// 窗口内误报总数达到此次数 → 自动切 relaxed 模式
    #[serde(default = "default_true")]
    pub auto_relax_mode: bool,
    #[serde(default = "default_relax_fp_count")]
    pub relax_after_fp_count: u32,
    #[serde(default = "default_window_hours")]
    pub window_hours: u32,
}

impl Default for SemanticAutoTune {
    fn default() -> Self {
        Self {
            enabled: true,
            false_positive_threshold: default_fp_threshold(),
            auto_whitelist_param: true,
            auto_relax_mode: true,
            relax_after_fp_count: default_relax_fp_count(),
            window_hours: default_window_hours(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SemanticWhitelist {
    #[serde(default)]
    pub param_names: Vec<String>,
    #[serde(default)]
    pub path_prefixes: Vec<String>,
    /// 匹配 Content-Type 前缀则跳过 body 切片
    #[serde(default)]
    pub content_types: Vec<String>,
}

/// 引擎组合 / 流水线优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineFusion {
    #[serde(default = "default_true")]
    pub semantic_first: bool,
    #[serde(default = "default_true")]
    pub libinjection: bool,
    #[serde(default = "default_true")]
    pub sql_intent: bool,
    #[serde(default = "default_true")]
    pub fallback_plain_libinjection: bool,
    #[serde(default = "default_pipeline_order")]
    pub pipeline_order: Vec<String>,
    /// 各检测段是否参与流水线（缺省或未配置视为启用）
    #[serde(default)]
    pub pipeline_enabled: HashMap<String, bool>,
}

impl EngineFusion {
    pub fn is_pipeline_stage_enabled(&self, stage: &str) -> bool {
        self.pipeline_enabled.get(stage).copied().unwrap_or(true)
    }

    /// 加载/保存前规范化阶段列表与开关（兼容历史 `crs`）。
    pub fn normalize_pipeline(&mut self) {
        self.pipeline_order = normalize_pipeline_order(&self.pipeline_order);
        self.pipeline_enabled =
            normalize_pipeline_enabled(&self.pipeline_order, &self.pipeline_enabled);
    }
}

impl Default for EngineFusion {
    fn default() -> Self {
        Self {
            semantic_first: true,
            libinjection: true,
            sql_intent: true,
            fallback_plain_libinjection: true,
            pipeline_order: default_pipeline_order(),
            pipeline_enabled: HashMap::new(),
        }
    }
}

fn default_pipeline_editor_mode() -> String {
    "list".into()
}

/// 流水线编排编辑器状态（Console 列表/流程图双模式）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineNodeLayout {
    pub x: i32,
    pub y: i32,
}

impl Default for PipelineNodeLayout {
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PipelineEditor {
    /// `list` | `flow`
    #[serde(default = "default_pipeline_editor_mode")]
    pub mode: String,
    /// 各检测段在编排画布上的偏移（像素，兼容旧版）
    #[serde(default)]
    pub layout: HashMap<String, PipelineNodeLayout>,
    /// LogicFlow 图数据（nodes + edges），Console 编排模式持久化
    #[serde(default)]
    pub graph: Option<serde_json::Value>,
}

/// 性能与采样
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticPerformance {
    #[serde(default = "default_max_slices")]
    pub max_slices_per_request: usize,
    #[serde(default = "default_sample_rate")]
    pub slow_path_sample_rate: u8,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u32,
}

impl Default for SemanticPerformance {
    fn default() -> Self {
        Self {
            max_slices_per_request: default_max_slices(),
            slow_path_sample_rate: default_sample_rate(),
            timeout_ms: default_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExpertLevel {
    Low,
    #[default]
    Medium,
    High,
}

impl ExpertLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" | "l" => Self::Low,
            "high" | "h" => Self::High,
            _ => Self::Medium,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: SemanticMode,
    #[serde(default)]
    pub decode: DecodePolicy,
    #[serde(default)]
    pub scope: DetectionScope,
    #[serde(default)]
    pub whitelist: SemanticWhitelist,
    #[serde(default)]
    pub engines: EngineFusion,
    #[serde(default)]
    pub performance: SemanticPerformance,
    #[serde(default)]
    pub auto_tune: SemanticAutoTune,
    #[serde(default = "default_true")]
    pub intent_enabled: bool,
    /// 检测流水线编排 UI（不影响运行时，仅 Console 持久化）
    #[serde(default)]
    pub pipeline_editor: PipelineEditor,
    /// 专家防护强度。全局必填语义（缺省 medium）；应用侧 `None` 表示继承全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expert_level: Option<ExpertLevel>,
}

impl Default for SemanticConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: SemanticMode::Standard,
            decode: DecodePolicy::default(),
            scope: DetectionScope::default(),
            whitelist: SemanticWhitelist::default(),
            engines: EngineFusion::default(),
            performance: SemanticPerformance::default(),
            auto_tune: SemanticAutoTune::default(),
            intent_enabled: true,
            pipeline_editor: PipelineEditor::default(),
            expert_level: Some(ExpertLevel::Medium),
        }
    }
}

impl SemanticConfig {
    pub fn effective_expert_level(&self, global: Option<ExpertLevel>) -> ExpertLevel {
        self.expert_level
            .or(global)
            .unwrap_or(ExpertLevel::Medium)
    }

    /// 历史：语义曾是流水线段。若显式关闭该段，迁移为关闭语义增强。
    fn migrate_semantic_stage_off(&mut self) {
        if self.engines.pipeline_enabled.get("semantic") == Some(&false) {
            self.enabled = false;
        }
    }

    pub fn normalized(mut self) -> Self {
        self.migrate_semantic_stage_off();
        if self.intent_enabled {
            self.engines.sql_intent = true;
        }
        self.intent_enabled = self.engines.sql_intent;
        self.engines.normalize_pipeline();
        self.performance.max_slices_per_request = self.performance.max_slices_per_request.clamp(1, 512);
        self.performance.slow_path_sample_rate = self.performance.slow_path_sample_rate.min(100);
        self.decode.max_depth = self.decode.max_depth.clamp(1, 32);
        self.auto_tune.false_positive_threshold = self.auto_tune.false_positive_threshold.max(1);
        self.auto_tune.relax_after_fp_count = self.auto_tune.relax_after_fp_count.max(1);
        self.auto_tune.window_hours = self.auto_tune.window_hours.clamp(1, 720);
        self
    }

    pub fn sql_intent_enabled(&self) -> bool {
        self.engines.sql_intent && self.intent_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_legacy_crs_to_expert_and_core() {
        let order = normalize_pipeline_order(&["semantic".into(), "crs".into(), "custom".into()]);
        assert_eq!(
            order,
            vec!["pre_access", "expert", "core", "custom"]
        );
        let enabled = normalize_pipeline_enabled(
            &order,
            &[("crs".into(), false)].into_iter().collect(),
        );
        assert_eq!(enabled.get("expert"), Some(&false));
        assert_eq!(enabled.get("core"), Some(&false));
        assert!(!enabled.contains_key("semantic"));
    }

    #[test]
    fn migrate_semantic_stage_off_disables_enhancement() {
        let mut cfg = SemanticConfig::default();
        cfg.engines.pipeline_enabled.insert("semantic".into(), false);
        cfg.engines.pipeline_order = vec![
            "semantic".into(),
            "expert".into(),
            "core".into(),
            "custom".into(),
        ];
        let cfg = cfg.normalized();
        assert!(!cfg.enabled);
        assert_eq!(
            cfg.engines.pipeline_order,
            vec!["pre_access", "expert", "core", "custom"]
        );
    }
}
