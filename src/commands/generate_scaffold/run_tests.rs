use super::*;

/// ```
/// // split_planning_variable extracts the first field name as planning variable.
/// ```
#[test]
fn test_split_no_fields() {
    let (pv, extra) = split_planning_variable(&[]);
    assert!(pv.is_none());
    assert!(extra.is_empty());
}

#[test]
fn test_split_one_field() {
    let fields = vec!["employee_idx:usize".to_string()];
    let (pv, extra) = split_planning_variable(&fields);
    assert_eq!(pv.as_deref(), Some("employee_idx"));
    assert!(extra.is_empty());
}

#[test]
fn test_split_multiple_fields() {
    let fields = vec!["employee_idx:usize".to_string(), "start:String".to_string()];
    let (pv, extra) = split_planning_variable(&fields);
    assert_eq!(pv.as_deref(), Some("employee_idx"));
    assert_eq!(extra, vec!["start:String"]);
}
