use crate::AppState;

/// 配置变更后 bump dataplane generation，通知 Edge Watch。
pub fn bump_after_mutation(state: &AppState) {
    let gen = crate::dataplane::bump_dataplane_generation(state);
    tracing::debug!(generation = gen, "dataplane generation bumped");
}
