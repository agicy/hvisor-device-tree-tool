use hvisor_device_tree_tool::dts::tree::{Data, Node, Property};
use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::filter::NodeFilter;
use visitors::writer::DtsWriter;

// Tests the node filtering logic.
//
// Verifies that nodes with `status = "disabled"` are correctly removed from the tree.
#[test]
fn test_filter() {
    let path = PathBuf::from("tests/data/test_filter.dts");
    let expected_path = PathBuf::from("tests/data/test_filter_expected.dts");
    let tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let predicate = |node: &Node| -> bool {
        if let Node::Existing { proplist, .. } = node {
            if let Some(Property::Existing {
                val: Some(data), ..
            }) = proplist.get("status")
            {
                for d in data {
                    if let Data::String(s) = d {
                        if s == "disabled" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    };
    let mut filter = NodeFilter::new(predicate);
    Walker::walk(&tree.root, "/", &mut filter);
    let filtered_root = filter.root.expect("Filter should produce a root");

    let mut buffer = Vec::new();
    {
        let mut writer = DtsWriter::new(&mut buffer, true);
        Walker::walk(&filtered_root, "/", &mut writer);
    }

    let output = String::from_utf8(buffer).expect("Invalid UTF-8");
    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    assert_eq!(output.trim(), expected.trim());
}
