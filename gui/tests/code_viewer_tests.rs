use std::path::PathBuf;
use to_markdown_gui::viewers::{CodeViewer, FileViewer};

#[test]
fn test_code_viewer_creates() {
    let path = PathBuf::from("test.rs");
    let language = "rust".to_string();
    let content = "fn main() {\n    println!(\"Hello, world!\");\n}".to_string();
    let dirty = false;

    let viewer = CodeViewer::new(path.clone(), language, content.clone(), dirty)
        .expect("Failed to create CodeViewer");

    // Verify the viewer was created successfully
    assert_eq!(viewer.get_content(), content, "Content should match");
    assert!(!viewer.is_dirty(), "Viewer should not be dirty initially");
    assert_eq!(viewer.file_type(), "code", "File type should be 'code'");
}

#[test]
fn test_code_viewer_render() {
    let path = PathBuf::from("test.rs");
    let language = "rust".to_string();
    let content = "fn main() {\n    println!(\"Hello\");\n}".to_string();
    let dirty = false;

    let viewer = CodeViewer::new(path, language, content, dirty)
        .expect("Failed to create CodeViewer");

    let html = viewer.render().expect("Failed to render");

    // Verify HTML structure
    assert!(html.contains("<pre"), "HTML should contain <pre> tag");
    assert!(html.contains("</pre>"), "HTML should contain closing </pre> tag");

    // Verify line numbers are present
    assert!(html.contains("class=\"line"), "HTML should contain line number classes");

    // Verify content is present
    assert!(html.contains("main"), "HTML should contain code content");
}

#[test]
fn test_code_viewer_dirty_flag() {
    let path = PathBuf::from("test.rs");
    let language = "rust".to_string();
    let content = "fn main() {}".to_string();
    let dirty = false;

    let mut viewer = CodeViewer::new(path, language, content, dirty)
        .expect("Failed to create CodeViewer");

    // Initial state should not be dirty
    assert!(!viewer.is_dirty(), "Viewer should not be dirty initially");

    let state = viewer.get_state();
    assert!(!state.modified, "State should show not modified");

    // Set dirty flag
    viewer.set_dirty(true);
    assert!(viewer.is_dirty(), "Viewer should be dirty after set_dirty(true)");

    let state = viewer.get_state();
    assert!(state.modified, "State should show modified");
    assert_eq!(state.file_type, "code", "State file_type should be 'code'");
}
