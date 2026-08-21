use crate::AppState;

/// After config change, bump dataplane generation and notify Edge Watch.
pub fn bump_after_mutation(state: &AppState) {
    let gen = crate::dataplane::bump_dataplane_generation(state);
    tracing::debug!(generation = gen, "dataplane generation bumped");
}
