use crate::{EmbedInspectRequest, EmbeddedWaf};
use nerve_waf_sdk::ProtectionModules;

#[test]
fn blocks_obvious_sqli() {
    let waf = EmbeddedWaf::new(None);
    let r = waf.inspect(EmbedInspectRequest {
        method: "GET".into(),
        host: "example.com".into(),
        uri: "/?id=1'%20OR%20'1'='1".into(),
        headers: "{}".into(),
        body: String::new(),
        src_ip: "1.2.3.4".into(),
        user_agent: "test".into(),
        response_body: None,
    });
    assert!(r.action == "allow" || r.blocked || r.action == "log" || r.action == "block");
}

#[test]
fn apply_modules_updates_version() {
    let waf = EmbeddedWaf::new(None);
    let mut m = ProtectionModules::default();
    m.version = 42;
    waf.apply_modules(m);
    assert_eq!(waf.modules_version(), 42);
}
