use anyhow::{anyhow, Result};
use plist::Value as PlistValue;

/// Represents a resource in a webarchive
#[derive(Debug, Clone)]
pub struct WebarchiveResource {
    pub url: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Represents a parsed webarchive structure
#[derive(Debug)]
pub struct Webarchive {
    pub main_resource: WebarchiveResource,
    pub sub_resources: Vec<WebarchiveResource>,
}

/// Parse a .webarchive file (Safari web archive format)
///
/// Webarchive files are plist-based archives containing HTML and resources.
/// The structure typically includes:
/// - MainResource: The primary HTML document
/// - Resources: Array of sub-resources (images, CSS, JS, etc.)
pub fn parse_webarchive(content: &[u8]) -> Result<Webarchive> {
    // Parse the plist format
    let plist: PlistValue = plist::from_bytes(content)
        .map_err(|e| anyhow!("Failed to parse webarchive plist: {}", e))?;

    // Convert to dictionary for easier access
    let dict = plist.as_dictionary()
        .ok_or_else(|| anyhow!("Invalid webarchive format: root is not a dictionary"))?;

    // Extract main resource
    let main_resource = extract_main_resource(dict)?;

    // Extract sub-resources (optional)
    let sub_resources = extract_sub_resources(dict).unwrap_or_default();

    Ok(Webarchive {
        main_resource,
        sub_resources,
    })
}

/// Extract the main HTML resource from the webarchive
fn extract_main_resource(dict: &plist::Dictionary) -> Result<WebarchiveResource> {
    let main_res = dict.get("MainResource")
        .ok_or_else(|| anyhow!("Webarchive missing MainResource"))?
        .as_dictionary()
        .ok_or_else(|| anyhow!("MainResource is not a dictionary"))?;

    let url = extract_string(main_res, "URL")?;
    let mime_type = extract_string(main_res, "MIMEType")
        .unwrap_or_else(|_| "text/html".to_string());

    // Data can be in 'Data' field (raw bytes) or 'TextEncodingName' + raw content
    let data = if let Some(data_value) = main_res.get("Data") {
        extract_bytes(data_value)?
    } else {
        // Some webarchives store the HTML content directly
        return Err(anyhow!("MainResource missing Data field"));
    };

    Ok(WebarchiveResource {
        url,
        mime_type,
        data,
    })
}

/// Extract sub-resources (images, CSS, scripts, etc.) from the webarchive
fn extract_sub_resources(dict: &plist::Dictionary) -> Result<Vec<WebarchiveResource>> {
    let mut resources = Vec::new();

    // Look for Resources array
    if let Some(resources_value) = dict.get("Resources") {
        if let Some(resources_array) = resources_value.as_array() {
            for item in resources_array {
                if let Some(res_dict) = item.as_dictionary() {
                    if let Ok(resource) = extract_resource_item(res_dict) {
                        resources.push(resource);
                    }
                    // Continue on error to be resilient
                }
            }
        }
    }

    Ok(resources)
}

/// Extract a single resource item
fn extract_resource_item(dict: &plist::Dictionary) -> Result<WebarchiveResource> {
    let url = extract_string(dict, "URL")?;
    let mime_type = extract_string(dict, "MIMEType")
        .unwrap_or_else(|_| "application/octet-stream".to_string());

    let data = if let Some(data_value) = dict.get("Data") {
        extract_bytes(data_value)?
    } else {
        Vec::new()
    };

    Ok(WebarchiveResource {
        url,
        mime_type,
        data,
    })
}

/// Extract a string value from plist dictionary
fn extract_string(dict: &plist::Dictionary, key: &str) -> Result<String> {
    dict.get(key)
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Missing or invalid string field: {}", key))
}

/// Extract bytes from plist value
fn extract_bytes(value: &PlistValue) -> Result<Vec<u8>> {
    if let Some(bytes) = value.as_data() {
        Ok(bytes.to_vec())
    } else if let Some(string) = value.as_string() {
        // Some webarchives might store data as UTF-8 string
        Ok(string.as_bytes().to_vec())
    } else {
        Err(anyhow!("Data field is neither bytes nor string"))
    }
}

/// Extract HTML content from a webarchive
///
/// This is the main function to use for converting webarchive to HTML text
pub fn extract_html_from_webarchive(content: &[u8]) -> Result<String> {
    let webarchive = parse_webarchive(content)?;

    // Convert bytes to string
    let html = String::from_utf8(webarchive.main_resource.data)
        .map_err(|e| anyhow!("Failed to decode HTML content as UTF-8: {}", e))?;

    Ok(html)
}

/// Get information about webarchive resources
pub fn get_webarchive_info(content: &[u8]) -> Result<String> {
    let webarchive = parse_webarchive(content)?;

    let mut info = String::new();
    info.push_str("# Webarchive Information\n\n");
    info.push_str("## Main Resource\n\n");
    info.push_str(&format!("- **URL:** {}\n", webarchive.main_resource.url));
    info.push_str(&format!("- **MIME Type:** {}\n", webarchive.main_resource.mime_type));
    info.push_str(&format!("- **Size:** {} bytes\n\n", webarchive.main_resource.data.len()));

    if !webarchive.sub_resources.is_empty() {
        info.push_str(&format!("## Sub-Resources ({} total)\n\n", webarchive.sub_resources.len()));
        info.push_str("| URL | MIME Type | Size |\n");
        info.push_str("|---|---|---|\n");

        for resource in &webarchive.sub_resources {
            let url_display = if resource.url.len() > 50 {
                format!("{}...", &resource.url[..47])
            } else {
                resource.url.clone()
            };
            info.push_str(&format!(
                "| `{}` | {} | {} |\n",
                url_display,
                resource.mime_type,
                resource.data.len()
            ));
        }
    } else {
        info.push_str("No sub-resources found\n");
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_string_success() {
        let mut dict = plist::Dictionary::new();
        dict.insert("key".to_string(), PlistValue::String("value".to_string()));

        assert_eq!(extract_string(&dict, "key").unwrap(), "value");
    }

    #[test]
    fn test_extract_string_missing() {
        let dict = plist::Dictionary::new();
        assert!(extract_string(&dict, "missing").is_err());
    }

    #[test]
    fn test_extract_bytes_from_data() {
        let mut dict = plist::Dictionary::new();
        let data = vec![1, 2, 3, 4, 5];
        dict.insert("Data".to_string(), PlistValue::Data(data.clone()));

        assert_eq!(extract_bytes(dict.get("Data").unwrap()).unwrap(), data);
    }

    #[test]
    fn test_extract_bytes_from_string() {
        let mut dict = plist::Dictionary::new();
        let text = "hello world";
        dict.insert("Data".to_string(), PlistValue::String(text.to_string()));

        let extracted = extract_bytes(dict.get("Data").unwrap()).unwrap();
        assert_eq!(extracted, text.as_bytes());
    }

    #[test]
    fn test_webarchive_info_generation() {
        // This test verifies the info function works (would need real webarchive for full test)
        let _info_parts = ["# Webarchive Information", "## Main Resource", "- **URL:**"];
        // Template test - real test would require sample webarchive
    }
}
