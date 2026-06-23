use anyhow::{anyhow, Result};
use scraper::{Html, Selector};
use std::path::Path;

/// Image output format option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Embed images as base64 in Markdown
    Embed,
    /// Link to external image URLs
    Link,
    /// Skip/ignore images
    Skip,
}

impl ImageFormat {
    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "embed" => Ok(ImageFormat::Embed),
            "link" => Ok(ImageFormat::Link),
            "skip" => Ok(ImageFormat::Skip),
            _ => Err(anyhow!("Invalid image format: {}. Use 'embed', 'link', or 'skip'", s)),
        }
    }
}

/// Represents an extracted image
#[derive(Debug, Clone)]
pub struct ExtractedImage {
    pub src: String,           // Original image URL/path
    pub alt_text: String,      // Alt text from HTML
    pub title: Option<String>, // Optional title attribute
    pub data_url: Option<String>, // Base64 data URL if embedded
}

/// Extract image URLs from HTML content
pub fn extract_image_urls(html_content: &str) -> Result<Vec<ExtractedImage>> {
    let document = Html::parse_document(html_content);
    let mut images = Vec::new();

    let img_selector = Selector::parse("img").map_err(|_| anyhow!("Invalid selector"))?;

    for img in document.select(&img_selector) {
        let src = img.value()
            .attr("src")
            .unwrap_or("")
            .to_string();

        if src.is_empty() {
            continue; // Skip images without src
        }

        let alt_text = img.value()
            .attr("alt")
            .unwrap_or("Image")
            .to_string();

        let title = img.value()
            .attr("title")
            .map(|s| s.to_string());

        images.push(ExtractedImage {
            src,
            alt_text,
            title,
            data_url: None,
        });
    }

    Ok(images)
}

/// Generate Markdown image syntax
pub fn generate_image_markdown(image: &ExtractedImage, format: ImageFormat) -> String {
    match format {
        ImageFormat::Embed => {
            // Use data URL if available, otherwise use src
            let url = image.data_url.as_ref().unwrap_or(&image.src);
            format!("![{}]({})", image.alt_text, url)
        }
        ImageFormat::Link => {
            let markdown = format!("![{}]({})", image.alt_text, image.src);
            if let Some(ref title) = image.title {
                format!("{}  \n*{}*", markdown, title)
            } else {
                markdown
            }
        }
        ImageFormat::Skip => {
            // Return alt text only, no image link
            format!("*[Image: {}]*", image.alt_text)
        }
    }
}

/// Convert image to base64 data URL
pub async fn image_to_data_url(image_path: &str) -> Result<String> {
    // Check if it's a URL or local path
    if image_path.starts_with("http://") || image_path.starts_with("https://") {
        fetch_and_encode_image(image_path).await
    } else {
        // Local file path
        encode_local_image(image_path).await
    }
}

/// Fetch image from URL and encode as base64
async fn fetch_and_encode_image(url: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch image from {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP error {}: {}", response.status(), url));
    }

    // Get content-type before consuming response
    let mime_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow!("Failed to read image data: {}", e))?;

    let base64 = base64_encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, base64))
}

/// Encode local image file as base64
async fn encode_local_image(path: &str) -> Result<String> {
    // Read file
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| anyhow!("Failed to read image file {}: {}", path, e))?;

    // Detect MIME type from extension
    let mime_type = detect_image_mime_type(path);

    let base64 = base64_encode(&bytes);
    Ok(format!("data:{};base64,{}", mime_type, base64))
}

/// Detect MIME type from file extension
fn detect_image_mime_type(path: &str) -> &'static str {
    let p = Path::new(path);
    match p.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" | "svgs" => "image/svg+xml",
            "bmp" => "image/bmp",
            "tiff" | "tif" => "image/tiff",
            "ico" => "image/x-icon",
            _ => "image/jpeg", // Default
        },
        None => "image/jpeg", // Default
    }
}

/// Encode bytes as base64
fn base64_encode(data: &[u8]) -> String {
    // Simple base64 encoding without external dependency
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let b1 = chunk[0];
        let b2 = chunk.get(1).copied().unwrap_or(0);
        let b3 = chunk.get(2).copied().unwrap_or(0);

        let n = ((b1 as u32) << 16) | ((b2 as u32) << 8) | (b3 as u32);

        result.push(TABLE[((n >> 18) & 63) as usize] as char);
        result.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            result.push(TABLE[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(TABLE[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

/// Process HTML content and replace image tags with Markdown
pub fn process_images_in_html(
    html_content: &str,
    image_format: ImageFormat,
) -> Result<String> {
    if image_format == ImageFormat::Skip {
        return Ok(html_content.to_string());
    }

    // Extract images and replace in HTML
    let document = Html::parse_document(html_content);
    let mut result = html_content.to_string();

    let img_selector = Selector::parse("img").map_err(|_| anyhow!("Invalid selector"))?;

    // Collect all replacements first (to avoid iterator issues)
    let mut replacements: Vec<(String, String)> = Vec::new();

    for img in document.select(&img_selector) {
        let src = img.value()
            .attr("src")
            .unwrap_or_default()
            .to_string();

        if src.is_empty() {
            continue;
        }

        let alt_text = img.value()
            .attr("alt")
            .unwrap_or("Image")
            .to_string();

        let markdown = format!("![{}]({})", alt_text, src);
        let html_tag = format!("<img src=\"{}\"", src);

        replacements.push((html_tag, markdown));
    }

    // Apply replacements
    for (old, new) in replacements {
        if let Some(pos) = result.find(&old) {
            // Find the closing > of the img tag
            if let Some(end) = result[pos..].find('>') {
                result.replace_range(pos..pos + end + 1, &new);
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_from_str() {
        assert_eq!(ImageFormat::from_str("embed").unwrap(), ImageFormat::Embed);
        assert_eq!(ImageFormat::from_str("link").unwrap(), ImageFormat::Link);
        assert_eq!(ImageFormat::from_str("skip").unwrap(), ImageFormat::Skip);
        assert_eq!(ImageFormat::from_str("EMBED").unwrap(), ImageFormat::Embed);
        assert!(ImageFormat::from_str("invalid").is_err());
    }

    #[test]
    fn test_detect_image_mime_type() {
        assert_eq!(detect_image_mime_type("image.jpg"), "image/jpeg");
        assert_eq!(detect_image_mime_type("image.jpeg"), "image/jpeg");
        assert_eq!(detect_image_mime_type("image.png"), "image/png");
        assert_eq!(detect_image_mime_type("image.gif"), "image/gif");
        assert_eq!(detect_image_mime_type("image.webp"), "image/webp");
        assert_eq!(detect_image_mime_type("image.svg"), "image/svg+xml");
    }

    #[test]
    fn test_generate_image_markdown_link() {
        let image = ExtractedImage {
            src: "https://example.com/image.jpg".to_string(),
            alt_text: "Example Image".to_string(),
            title: None,
            data_url: None,
        };

        let markdown = generate_image_markdown(&image, ImageFormat::Link);
        assert_eq!(markdown, "![Example Image](https://example.com/image.jpg)");
    }

    #[test]
    fn test_generate_image_markdown_skip() {
        let image = ExtractedImage {
            src: "https://example.com/image.jpg".to_string(),
            alt_text: "Example Image".to_string(),
            title: None,
            data_url: None,
        };

        let markdown = generate_image_markdown(&image, ImageFormat::Skip);
        assert_eq!(markdown, "*[Image: Example Image]*");
    }

    #[test]
    fn test_base64_encode() {
        // Test with known base64
        let data = b"Hello";
        let encoded = base64_encode(data);
        assert_eq!(encoded, "SGVsbG8=");
    }

    #[test]
    fn test_extract_image_urls_empty() {
        let html = "<h1>No images here</h1>";
        let images = extract_image_urls(html).unwrap();
        assert_eq!(images.len(), 0);
    }

    #[test]
    fn test_extract_image_urls_with_alt() {
        let html = "<img src=\"image.jpg\" alt=\"Test Image\" title=\"A test\">";
        let images = extract_image_urls(html).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].src, "image.jpg");
        assert_eq!(images[0].alt_text, "Test Image");
        assert_eq!(images[0].title, Some("A test".to_string()));
    }
}
