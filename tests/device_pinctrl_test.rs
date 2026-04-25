use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::device_pinctrl::DevicePinctrlExtractor;

#[test]
fn test_device_pinctrl() {
    let path = PathBuf::from("tests/data/test_device_pinctrl.dts");
    let expected_path = PathBuf::from("tests/data/test_device_pinctrl_expected.txt");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    // Note: DevicePinctrlExtractor needs reference to the tree for lookup
    let mut extractor = DevicePinctrlExtractor::new(&tree);
    Walker::walk(&tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");
    
    // Normalize newlines and trim
    assert_eq!(extractor.output().trim(), expected.trim());
}
