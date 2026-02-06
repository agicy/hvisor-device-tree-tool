use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::interrupts::InterruptsExtractor;

// Tests the interrupt information extraction logic.
//
// Verifies that interrupt parents and controllers are correctly resolved
// and that the output matches the expected format.
#[test]
fn test_interrupts() {
    let path = PathBuf::from("tests/data/test_interrupts.dts");
    let expected_path = PathBuf::from("tests/data/test_interrupts_expected.txt");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut extractor = InterruptsExtractor::new();
    Walker::walk(&tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    assert_eq!(extractor.output().trim(), expected.trim());
}
