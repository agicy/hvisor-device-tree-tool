use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::dependency::DependencyExtractor;

#[test]
fn test_dependencies() {
    let path = PathBuf::from("tests/data/test_dependencies.dts");
    let expected_path = PathBuf::from("tests/data/test_dependencies_expected.txt");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut extractor = DependencyExtractor::new();
    Walker::walk(&tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");
    
    assert_eq!(extractor.output().trim(), expected.trim());
}
