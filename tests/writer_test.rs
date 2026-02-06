use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::writer::DtsWriter;

// Tests the DTS writing logic.
//
// Verifies that the tree is correctly serialized back to DTS format.
#[test]
fn test_writer() {
    let path = PathBuf::from("tests/data/test_writer.dts");
    let expected_path = PathBuf::from("tests/data/test_writer_expected.dts");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut buffer = Vec::new();
    {
        let mut writer = DtsWriter::new(&mut buffer, true);
        Walker::walk(&tree.root, "/", &mut writer);
    }

    let output = String::from_utf8(buffer).expect("Invalid UTF-8");
    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    assert_eq!(output.trim(), expected.trim());
}
