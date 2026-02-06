use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::sorter::SortByReference;
use visitors::writer::DtsWriter;

// Tests the node sorting logic.
//
// Verifies that nodes are topologically sorted based on their phandle references.
#[test]
fn test_sorter() {
    let path = PathBuf::from("tests/data/test_sorter.dts");
    let expected_path = PathBuf::from("tests/data/test_sorter_expected.dts");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    // 1. Sort the tree (creates new tree)
    let mut sorter = SortByReference::new();
    Walker::walk(&tree.root, "/", &mut sorter);
    let sorted_root = sorter.root.expect("Sorter should produce a root");

    // 2. Write the tree to a string
    let mut output = Vec::new();
    let mut writer = DtsWriter::new(&mut output, true); // Use tabs to match source
    Walker::walk(&sorted_root, "/", &mut writer);

    let output_str = String::from_utf8(output).expect("Invalid UTF-8 output");

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    // Normalize newlines and trim for comparison
    assert_eq!(output_str.trim(), expected.trim());
}
