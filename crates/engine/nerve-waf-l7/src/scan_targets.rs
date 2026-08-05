use crate::InspectRequest;
use serde::{Deserialize, Serialize};

/// 与 ModSecurity CRS 变量对齐的扫描目标（不含原始 headers 全文）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScanTarget {
    Args,
    ArgsNames,
    Cookies,
    CookieNames,
    UserAgent,
    Referer,
    RequestFilename,
    RequestBasename,
    Body,
}

impl ScanTarget {
    pub fn label(self) -> &'static str {
        match self {
            Self::Args => "ARGS",
            Self::ArgsNames => "ARGS_NAMES",
            Self::Cookies => "COOKIES",
            Self::CookieNames => "COOKIE_NAMES",
            Self::UserAgent => "User-Agent",
            Self::Referer => "Referer",
            Self::RequestFilename => "REQUEST_FILENAME",
            Self::RequestBasename => "REQUEST_BASENAME",
            Self::Body => "BODY",
        }
    }

    pub fn from_crs_token(token: &str) -> Option<Self> {
        let t = token.trim().to_ascii_uppercase();
        if t.starts_with("XML:") {
            return Some(Self::Body);
        }
        match t.as_str() {
            "ARGS" => Some(Self::Args),
            "ARGS_NAMES" => Some(Self::ArgsNames),
            "REQUEST_COOKIES" | "COOKIES" => Some(Self::Cookies),
            "REQUEST_COOKIES_NAMES" | "COOKIE_NAMES" => Some(Self::CookieNames),
            "REQUEST_HEADERS:REFERER" | "REFERER" => Some(Self::Referer),
            "REQUEST_HEADERS:USER-AGENT" | "USER-AGENT" | "USERAGENT" => Some(Self::UserAgent),
            "REQUEST_FILENAME" => Some(Self::RequestFilename),
            "REQUEST_BASENAME" => Some(Self::RequestBasename),
            "BODY" | "REQUEST_BODY" => Some(Self::Body),
            "REQUEST_URI" | "REQUEST_LINE" => Some(Self::Args),
            _ if t.starts_with("REQUEST_HEADERS:") => {
                // 仅支持明确映射的 header；其余 REQUEST_HEADERS:Foo 不纳入通用扫描
                None
            }
            _ => None,
        }
    }
}

/// 无 scan_targets 时：对齐 CRS 常见 @rx 目标（ARGS/COOKIE），不扫 UA/Referer/路径。
pub const DEFAULT_SCAN_TARGETS: &[ScanTarget] = &[
    ScanTarget::Args,
    ScanTarget::ArgsNames,
    ScanTarget::Cookies,
    ScanTarget::CookieNames,
    ScanTarget::Body,
];

/// 历史兼容：曾跳过无 chain 引擎下会误报的规则 ID。现已支持 chain，恒为 false。
pub const CHAIN_ONLY_RULE_IDS: &[&str] = &[];

pub fn is_chain_only_rule(_id: &str) -> bool {
    false
}

pub fn parse_scan_targets(raw: &[String]) -> Vec<ScanTarget> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for token in raw {
        if let Some(t) = ScanTarget::from_crs_token(token) {
            if seen.insert(t) {
                out.push(t);
            }
        }
    }
    out
}

pub fn resolve_scan_targets(raw: Option<&[String]>) -> Vec<ScanTarget> {
    match raw {
        Some(list) if !list.is_empty() => parse_scan_targets(list),
        _ => DEFAULT_SCAN_TARGETS.to_vec(),
    }
}

pub struct RequestSamples {
    query_values: Vec<String>,
    query_names: Vec<String>,
    body_form_values: Vec<String>,
    body: String,
    body_decoded: String,
    user_agent: String,
    referer: String,
    cookies: Vec<String>,
    cookie_names: Vec<String>,
    filename: String,
    basename: String,
}

impl RequestSamples {
    pub fn from(req: &InspectRequest) -> Self {
        let body_decoded = decode_input(&req.body);
        let (query_names, query_values) = parse_query(&req.uri);
        let (_, body_form_values) = parse_form_values(&req.body);
        let (cookie_names, cookies) = parse_cookies(&req.headers);
        let referer = header_value(&req.headers, "referer");
        let filename = path_from_uri(&req.uri);
        let basename = filename
            .rsplit('/')
            .next()
            .unwrap_or(&filename)
            .to_string();
        Self {
            query_values,
            query_names,
            body_form_values,
            body: req.body.clone(),
            body_decoded,
            user_agent: req.user_agent.clone(),
            referer,
            cookies,
            cookie_names,
            filename,
            basename,
        }
    }

    pub fn for_targets(&self, targets: &[ScanTarget]) -> Vec<(&'static str, &str)> {
        targets
            .iter()
            .flat_map(|t| self.values_for(*t))
            .collect()
    }

    fn values_for<'a>(&'a self, target: ScanTarget) -> Vec<(&'static str, &'a str)> {
        let mut out = Vec::new();
        match target {
            ScanTarget::Args => {
                for v in &self.query_values {
                    if !v.is_empty() {
                        out.push(("ARGS", v.as_str()));
                    }
                }
                for v in &self.body_form_values {
                    if !v.is_empty() {
                        out.push(("ARGS(body)", v.as_str()));
                    }
                }
            }
            ScanTarget::ArgsNames => {
                for n in &self.query_names {
                    out.push(("ARGS_NAMES", n.as_str()));
                }
            }
            ScanTarget::Cookies => {
                for c in &self.cookies {
                    out.push(("COOKIES", c.as_str()));
                }
            }
            ScanTarget::CookieNames => {
                for n in &self.cookie_names {
                    out.push(("COOKIE_NAMES", n.as_str()));
                }
            }
            ScanTarget::UserAgent => {
                if !self.user_agent.is_empty() {
                    out.push(("User-Agent", self.user_agent.as_str()));
                }
            }
            ScanTarget::Referer => {
                if !self.referer.is_empty() {
                    out.push(("Referer", self.referer.as_str()));
                }
            }
            ScanTarget::RequestFilename => {
                out.push(("REQUEST_FILENAME", self.filename.as_str()));
            }
            ScanTarget::RequestBasename => {
                out.push(("REQUEST_BASENAME", self.basename.as_str()));
            }
            ScanTarget::Body => {
                if !self.body.is_empty() {
                    out.push(("BODY", self.body.as_str()));
                }
                if !self.body_decoded.is_empty() && self.body_decoded != self.body {
                    out.push(("BODY(decoded)", self.body_decoded.as_str()));
                }
            }
        }
        out
    }
}

fn decode_input(s: &str) -> String {
    let plus = s.replace('+', " ");
    urlencoding::decode(&plus)
        .map(|c| c.into_owned())
        .unwrap_or(plus)
}

fn parse_form_values(body: &str) -> (Vec<String>, Vec<String>) {
    if body.is_empty() || body.starts_with('{') {
        return (Vec::new(), Vec::new());
    }
    parse_query(&format!("?{body}"))
}

fn parse_query(uri: &str) -> (Vec<String>, Vec<String>) {
    let q = uri.split('?').nth(1).unwrap_or("");
    let mut names = Vec::new();
    let mut values = Vec::new();
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => {
                names.push(decode_input(k));
                values.push(decode_input(v));
            }
            None => {
                names.push(decode_input(pair));
                values.push(String::new());
            }
        }
    }
    (names, values)
}

fn parse_cookies(headers: &str) -> (Vec<String>, Vec<String>) {
    let mut names = Vec::new();
    let mut values = Vec::new();
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.starts_with("cookie:") {
            continue;
        }
        let rest = line.split_once(':').map(|(_, v)| v.trim()).unwrap_or("");
        for part in rest.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.split_once('=') {
                Some((k, v)) => {
                    names.push(k.trim().into());
                    values.push(decode_input(v.trim()));
                }
                None => names.push(part.into()),
            }
        }
    }
    (names, values)
}

fn header_value(headers: &str, name: &str) -> String {
    let needle = format!("{name}:");
    for line in headers.lines() {
        if line.len() >= needle.len() && line[..needle.len()].eq_ignore_ascii_case(&needle) {
            return line[needle.len()..].trim().into();
        }
    }
    String::new()
}

fn path_from_uri(uri: &str) -> String {
    let path = uri.split('?').next().unwrap_or(uri);
    if path.is_empty() {
        "/".into()
    } else {
        path.into()
    }
}
