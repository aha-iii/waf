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

/// Canonical pipeline stage ids (aligned with Console `pipeline-stages.ts`).
/// Semantic is enhancement config, not a reorderable detect segment.
pub const PIPELINE_STAGE_IDS: &[&str] = &["pre_access", "expert", "core", "custom"];

fn default_pipeline_order() -> Vec<String> {
    PIPELINE_STAGE_IDS.iter().map(|s| (*s).into()).collect()
}

/// Expand historical `crs` into `expert` + `core`; strip deprecated `semantic`/`plugin`; fill missing stages.
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
        let canon_idx = PIPELINE_STAGE_IDS
            .iter()
            .position(|s| *s == *id)
            .unwrap_or(0);
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

/// Migrate `pipeline_enabled`: copy old key `crs` to expert/core.
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

/// Feedback-driven auto-tune (long-term ops).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticAutoTune {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Same param_path FP count reaches this → auto-add to param allowlist.
    #[serde(default = "default_fp_threshold")]
    pub false_positive_threshold: u32,
    #[serde(default = "default_true")]
    pub auto_whitelist_param: bool,
    /// Window FP total reaches this → auto-switch to relaxed mode.
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
    /// Skip body slicing when Content-Type prefix matches.
    #[serde(default)]
    pub content_types: Vec<String>,
}

/// Engine composition / pipeline priority.
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
    /// Whether each detect segment participates (missing/unset = enabled).
    #[serde(default)]
    pub pipeline_enabled: HashMap<String, bool>,
}

impl EngineFusion {
    pub fn is_pipeline_stage_enabled(&self, stage: &str) -> bool {
        self.pipeline_enabled.get(stage).copied().unwrap_or(true)
    }

    /// Normalize stage list and switches before load/save (compat historical `crs`).
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

/// Pipeline orchestration editor state (Console list/flowchart dual mode).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PipelineNodeLayout {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PipelineEditor {
    /// `list` | `flow`
    #[serde(default = "default_pipeline_editor_mode")]
    pub mode: String,
    /// Per-segment offsets on orchestration canvas (pixels; legacy compat).
    #[serde(default)]
    pub layout: HashMap<String, PipelineNodeLayout>,
    /// LogicFlow graph data (nodes + edges); persisted in Console orchestration mode.
    #[serde(default)]
    pub graph: Option<serde_json::Value>,
}

/// Performance and sampling.
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
    /// Detect-pipeline orchestration UI (Console persistence only; no runtime effect).
    #[serde(default)]
    pub pipeline_editor: PipelineEditor,
    /// Expert protection strength. Required globally (default medium); app-side `None` means inherit global.
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
        self.expert_level.or(global).unwrap_or(ExpertLevel::Medium)
    }

    /// Historical: semantic used to be a pipeline segment. Explicitly disabling it migrates to disable semantic enhancement.
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
        self.performance.max_slices_per_request =
            self.performance.max_slices_per_request.clamp(1, 512);
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
