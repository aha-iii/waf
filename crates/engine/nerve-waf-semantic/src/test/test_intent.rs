//! Tests extracted from `intent/intent.rs`.

use crate::intent::*;

#[test]
fn parses_union_select() {
    let (intent, score) = classify_sql_intent("1 union select null,null");
    assert!(matches!(intent, SqlIntent::Query | SqlIntent::Unknown));
    assert!(score >= 40);
}
