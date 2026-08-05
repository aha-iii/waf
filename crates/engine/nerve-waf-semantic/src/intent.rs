use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlIntent {
    Benign,
    Query,
    Modify,
    Execute,
    Unknown,
}

pub fn intent_label(intent: SqlIntent) -> &'static str {
    match intent {
        SqlIntent::Benign => "benign",
        SqlIntent::Query => "query",
        SqlIntent::Modify => "modify",
        SqlIntent::Execute => "execute",
        SqlIntent::Unknown => "unknown",
    }
}

pub fn classify_sql_intent(text: &str) -> (SqlIntent, u8) {
    let lower = text.to_ascii_lowercase();
    if !looks_like_sql_fragment(&lower) {
        return (SqlIntent::Benign, 0);
    }

    let dialect = GenericDialect {};
    if let Ok(stmts) = Parser::parse_sql(&dialect, text) {
        if !stmts.is_empty() {
            let intent = intent_from_statements(&stmts);
            let score = match intent {
                SqlIntent::Execute => 90,
                SqlIntent::Modify => 85,
                SqlIntent::Query => 70,
                SqlIntent::Benign => 20,
                SqlIntent::Unknown => 50,
            };
            return (intent, score);
        }
    }

    keyword_intent(&lower)
}

fn looks_like_sql_fragment(lower: &str) -> bool {
    [
        "select", "union", "insert", "update", "delete", "drop", "alter", "create",
        "sleep", "benchmark", "load_file", "information_schema", " or ", " and ",
        "' or", "1=1", "0x",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

fn intent_from_statements(stmts: &[sqlparser::ast::Statement]) -> SqlIntent {
    use sqlparser::ast::Statement;
    for stmt in stmts {
        match stmt {
            Statement::Query(_) => return SqlIntent::Query,
            Statement::Insert { .. }
            | Statement::Update { .. }
            | Statement::Delete { .. }
            | Statement::Merge { .. } => return SqlIntent::Modify,
            Statement::Drop { .. }
            | Statement::AlterTable { .. }
            | Statement::CreateTable { .. } => return SqlIntent::Execute,
            _ => {}
        }
    }
    SqlIntent::Unknown
}

fn keyword_intent(lower: &str) -> (SqlIntent, u8) {
    if lower.contains("union select") || lower.contains(" or 1=1") || lower.contains("' or '") {
        return (SqlIntent::Query, 75);
    }
    if lower.contains("insert into") || lower.contains("update ") || lower.contains("delete from") {
        return (SqlIntent::Modify, 80);
    }
    if lower.contains("drop table") || lower.contains("sleep(") || lower.contains("benchmark(") {
        return (SqlIntent::Execute, 85);
    }
    if lower.contains("select ") {
        return (SqlIntent::Query, 65);
    }
    (SqlIntent::Unknown, 40)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_union_select() {
        let (intent, score) = classify_sql_intent("1 union select null,null");
        assert!(matches!(intent, SqlIntent::Query | SqlIntent::Unknown));
        assert!(score >= 40);
    }
}
