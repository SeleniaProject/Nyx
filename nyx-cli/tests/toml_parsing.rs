#![forbid(unsafe_code)]

#[test]
fn negative_timeout_is_ignored() {
    let toml = r#"
[cli]
request_timeout_m_s = -10
"#;
    let parsed: toml::Value = toml::from_str(toml)?;
    let cli = parsed.get("cli")?;
    let v = cli.get("request_timeout_m_s").and_then(|x| x.as_integer());
    assert_eq!(v, Some(-10));
}
