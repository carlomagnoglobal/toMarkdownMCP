use std::path::PathBuf;
use to_markdown_gui::viewers::{FileViewer, ImageViewer};

#[test]
fn test_image_viewer_creates() {
    let path = PathBuf::from("test.png");
    let format = "png".to_string();
    let width = 800;
    let height = 600;

    let viewer = ImageViewer::new(path.clone(), format.clone(), width, height)
        .expect("Failed to create ImageViewer");

    // Verify the viewer was created successfully
    assert_eq!(viewer.file_type(), "image", "File type should be 'image'");

    let state = viewer.get_state();
    assert_eq!(state.file_type, "image", "State file_type should be 'image'");
    assert_eq!(state.file_path, path, "File path should match");
    assert!(!state.modified, "Image viewer should not be modified");
}

#[test]
fn test_image_viewer_dimensions() {
    let path = PathBuf::from("photo.jpeg");
    let format = "jpeg".to_string();
    let width = 1920;
    let height = 1080;
    let file_size = 256000;

    let viewer = ImageViewer::new_with_size(path, format.clone(), width, height, file_size)
        .expect("Failed to create ImageViewer with size");

    let state = viewer.get_state();
    assert_eq!(state.file_size_bytes, file_size, "File size should match");
    assert_eq!(state.file_type, "image", "File type should be 'image'");
}

#[test]
fn test_image_viewer_render() {
    let path = PathBuf::from("image.svg");
    let format = "svg".to_string();
    let width = 512;
    let height = 512;
    let file_size = 50000;

    let viewer = ImageViewer::new_with_size(path.clone(), format.clone(), width, height, file_size)
        .expect("Failed to create ImageViewer");

    let html = viewer.render().expect("Failed to render");

    // Verify HTML structure
    assert!(html.contains("<img"), "HTML should contain <img> tag");
    assert!(html.contains("file://"), "HTML should contain file:// URL");
    assert!(html.contains("max-width: 100%"), "HTML should have max-width styling");

    // Verify metadata is present
    assert!(html.contains("Format:"), "HTML should display Format");
    assert!(html.contains("Dimensions:"), "HTML should display Dimensions");
    assert!(html.contains("File Size:"), "HTML should display File Size");

    // Verify format and dimensions are rendered
    assert!(html.contains("SVG"), "HTML should contain uppercase format");
    assert!(html.contains("512 x 512"), "HTML should contain dimensions");
    assert!(html.contains("50000 bytes"), "HTML should contain file size");
}
