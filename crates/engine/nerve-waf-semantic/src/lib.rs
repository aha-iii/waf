//! 语义检测：参数定位 → 递归解码 → libinjection + SQL 意图 → 融合输出

mod auto_tune;
mod config;
mod inspect;
mod intent;
mod stats;

pub use auto_tune::{apply_auto_tune, AutoTuneResult};
pub use config::{
    normalize_pipeline_enabled, normalize_pipeline_order, EngineFusion, ExpertLevel, PipelineEditor,
    PipelineNodeLayout, SemanticAutoTune, SemanticConfig, SemanticMode, SemanticPerformance,
    SemanticWhitelist, PIPELINE_STAGE_IDS,
};
pub use inspect::inspect_semantic;
pub use stats::{
    now_iso, InMemorySemanticStats, InspectTimer, SemanticCounters, SemanticFeedback,
    SemanticSample, SemanticStatsHandle, SemanticStatsRecorder, SemanticStatsSnapshot,
};
