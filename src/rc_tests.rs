use super::*;

/// ```
/// // RcConfig parses known TOML keys correctly.
/// ```
#[test]
fn test_parse_rc_full() {
    let toml = r#"
port = 8080
no_color = true
quiet = false
"#;
    let cfg = parse_rc(toml).unwrap();
    assert_eq!(cfg.port, Some(8080));
    assert!(cfg.no_color);
    assert!(!cfg.quiet);
}

#[test]
fn test_parse_rc_empty() {
    let cfg = parse_rc("").unwrap();
    assert!(cfg.is_empty());
}

#[test]
fn test_parse_rc_invalid_port() {
    let cfg = parse_rc("port = 99999").unwrap();
    assert_eq!(cfg.port, None);
}

#[test]
fn test_parse_rc_bad_toml() {
    let cfg = parse_rc("not valid [ toml").unwrap();
    assert!(cfg.is_empty());
}
