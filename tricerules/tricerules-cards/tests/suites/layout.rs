//! Keep explicit Cargo suites from silently dropping a newly added test file.
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[test]
fn every_card_test_source_belongs_to_exactly_one_suite() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut membership = BTreeMap::new();
    for suite in [
        "batch_registry",
        "deck_registry",
        "generated_registry",
        "mechanic_registry",
        "standard_registry",
    ] {
        let source = fs::read_to_string(tests.join("suites").join(format!("{suite}.rs")))
            .expect("every declared card suite must exist");
        for line in source.lines() {
            if let Some(file) = line
                .strip_prefix("#[path = \"../")
                .and_then(|line| line.strip_suffix("\"]"))
            {
                assert!(tests.join(file).is_file(), "missing test source: {file}");
                *membership.entry(file.to_owned()).or_insert(0) += 1;
            }
        }
    }
    for entry in fs::read_dir(tests).expect("card test sources") {
        let path = entry.expect("test source entry").path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let file = path.file_name().unwrap().to_str().unwrap();
            assert_eq!(
                membership.get(file),
                Some(&1),
                "add {file} to exactly one tests/suites registry harness"
            );
        }
    }
}
