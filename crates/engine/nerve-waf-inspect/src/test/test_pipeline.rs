//! Tests extracted from `pipeline/pipeline.rs`.

use crate::pipeline::soften_action_for_detect_mode;
use nerve_waf_core::WafAction;

#[test]
fn detect_mode_softens_block_and_challenge() {
    assert_eq!(
        soften_action_for_detect_mode("detect", WafAction::Block),
        WafAction::Log
    );
    assert_eq!(
        soften_action_for_detect_mode("DETECT", WafAction::Challenge),
        WafAction::Log
    );
    assert_eq!(
        soften_action_for_detect_mode("detect", WafAction::Log),
        WafAction::Log
    );
}

#[test]
fn block_mode_keeps_enforcement() {
    assert_eq!(
        soften_action_for_detect_mode("block", WafAction::Block),
        WafAction::Block
    );
    assert_eq!(
        soften_action_for_detect_mode("block", WafAction::Challenge),
        WafAction::Challenge
    );
}
