//! Scenario tests: raw terminal bytes through the real input decoder into the core.

use xplain_sim::Sim;

#[test]
fn bytes_shift_f_opens_search() {
    let mut s = Sim::builder().size(100, 30).build();
    s.bytes(b"F");
    s.assert_contains(" Search (");
    s.bytes(b"\x1b");
    s.assert_not_contains(" Search (");
}

#[test]
fn bytes_lone_esc_closes_picker_without_next_key() {
    let mut s = Sim::builder().size(100, 30).build();
    s.bytes(b"f");
    s.assert_contains(" Files (");
    s.bytes(b"\x1b");
    s.assert_not_contains(" Files (");
}

#[test]
fn bytes_esc_then_shift_f_is_not_alt_f() {
    let mut s = Sim::builder().size(100, 30).build();
    s.bytes(b"\x1b").bytes(b"F");
    s.assert_contains(" Search (");
}

#[test]
fn bytes_several_keys_in_one_read() {
    let mut s = Sim::builder().size(100, 30).build();
    s.bytes(b"F\x1b?\x1b");
    s.assert_not_contains(" Search (");
}

#[test]
fn bytes_csi_u_shift_f_opens_search() {
    let mut s = Sim::builder().size(100, 30).build();
    s.bytes(b"\x1b[102;2u");
    s.assert_contains(" Search (");
}
