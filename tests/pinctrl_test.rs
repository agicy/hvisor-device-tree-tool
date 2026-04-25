use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::pinctrl::PinctrlExtractor;

#[test]
fn test_pinctrl() {
    let path = PathBuf::from("tests/data/test_pinctrl.dts");
    let expected_path = PathBuf::from("tests/data/test_pinctrl_expected.txt");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut extractor = PinctrlExtractor::new();
    Walker::walk(&tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");
    
    // Normalize newlines and trim
    assert_eq!(extractor.output().trim(), expected.trim());
}
