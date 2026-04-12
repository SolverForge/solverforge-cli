use super::remove_constraint_call_from_line;

#[test]
fn removes_constraint_from_multiline_tuple_entry() {
    let line = "            all_assigned::constraint(),";
    let updated =
        remove_constraint_call_from_line(line, "all_assigned").expect("line should be rewritten");
    assert!(updated.is_empty());
}

#[test]
fn removes_constraint_from_flat_tuple_line() {
    let line = "    (capacity::constraint(), extra::constraint(), distance::constraint())";
    let updated =
        remove_constraint_call_from_line(line, "extra").expect("line should be rewritten");
    assert_eq!(
        updated,
        "    (capacity::constraint(), distance::constraint())"
    );
}
