use super::helpers::{authoring_rows::run_rows, semantic::main_phase};

const SKULLCLAMP_ROW: &str = r#"[
  {"family":"skullclamp_attached_creature_dies_draw"}
]"#;

#[test]
fn skullclamp_equips_and_draws_when_equipped_creature_dies() {
    let exercised = run_rows(SKULLCLAMP_ROW, || main_phase(659_860));
    assert_eq!(exercised, ["skullclamp".to_owned()].into());
}
