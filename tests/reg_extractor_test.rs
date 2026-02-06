use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::reg_extractor::RegExtractor;

// Tests the register information extraction logic.
//
// Verifies that `#address-cells` and `#size-cells` are correctly handled
// and that register ranges are extracted accurately.
#[test]
fn test_reg_extractor() {
    let path = PathBuf::from("tests/data/test_reg_extractor.dts");
    let expected_path = PathBuf::from("tests/data/test_reg_extractor_expected.txt");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut extractor = RegExtractor::new();
    Walker::walk(&tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    assert_eq!(extractor.output().trim(), expected.trim());
}
