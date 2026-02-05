use hvisor_device_tree_tool::dts::tree::{Data, Node, Property};
use hvisor_device_tree_tool::{dts, visitors};
use std::path::PathBuf;
use visitors::Walker;
use visitors::filter::NodeFilter;
use visitors::writer::DtsWriter;
use visitors::reg_extractor::RegExtractor;
use visitors::interrupts::InterruptsExtractor;

#[test]
fn test_reg_extractor() {
    let path = PathBuf::from("tests/data/test_reg_extractor.dts");
    let expected_path = PathBuf::from("tests/data/test_reg_extractor_expected.txt");
    let mut tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut extractor = RegExtractor::new();
    Walker::walk(&mut tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");
    
    assert_eq!(extractor.output.trim(), expected.trim());
}

#[test]
fn test_interrupts() {
    let path = PathBuf::from("tests/data/test_interrupts.dts");
    let expected_path = PathBuf::from("tests/data/test_interrupts_expected.txt");
    let mut tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut extractor = InterruptsExtractor::new();
    Walker::walk(&mut tree.root, "/", &mut extractor);

    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");
    
    assert_eq!(extractor.output.trim(), expected.trim());
}

#[test]
fn test_writer() {
    let path = PathBuf::from("tests/data/test_writer.dts");
    let expected_path = PathBuf::from("tests/data/test_writer_expected.dts");
    let mut tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

    let mut buffer = Vec::new();
    {
        let mut writer = DtsWriter::new(&mut buffer, true, 4);
        Walker::walk(&mut tree.root, "/", &mut writer);
    }

    let output = String::from_utf8(buffer).expect("Invalid UTF-8");
    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    assert_eq!(output.trim(), expected.trim());
}

#[test]
fn test_filter() {
    let path = PathBuf::from("tests/data/test_filter.dts");
    let expected_path = PathBuf::from("tests/data/test_filter_expected.dts");
    let mut tree = dts::parse_dts(Some(&path)).expect("Failed to parse DTS");

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
    Walker::walk(&mut tree.root, "/", &mut filter);

    let mut buffer = Vec::new();
    {
        let mut writer = DtsWriter::new(&mut buffer, true, 4);
        Walker::walk(&mut tree.root, "/", &mut writer);
    }

    let output = String::from_utf8(buffer).expect("Invalid UTF-8");
    let expected = std::fs::read_to_string(&expected_path).expect("Failed to read expected file");

    assert_eq!(output.trim(), expected.trim());
}
