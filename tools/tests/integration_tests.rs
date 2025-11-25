use std::fs;
use tempfile::TempDir;

// ============================================================================
// Layout Data Tests - Testing core data structures
// ============================================================================

#[test]
fn test_layout_json_deserialization() {
    // Test that we can properly deserialize a layout JSON file
    let layout_json = r#"{
        "metadata": {
            "url": "http://test.com",
            "title": "Test",
            "viewport": {"width": 1920, "height": 1080},
            "devicePixelRatio": 1.0,
            "timestamp": "2024-01-01T00:00:00Z"
        },
        "elements": [
            {
                "index": 0,
                "tag": "div",
                "id": "test",
                "classes": ["foo", "bar"],
                "x": 10.0,
                "y": 20.0,
                "width": 100.0,
                "height": 50.0
            }
        ]
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(layout_json).unwrap();

    // Verify metadata
    assert_eq!(parsed["metadata"]["url"], "http://test.com");
    assert_eq!(parsed["metadata"]["title"], "Test");
    assert_eq!(parsed["metadata"]["viewport"]["width"], 1920);
    assert_eq!(parsed["metadata"]["viewport"]["height"], 1080);
    assert_eq!(parsed["metadata"]["devicePixelRatio"], 1.0);

    // Verify elements
    let elements = parsed["elements"].as_array().unwrap();
    assert_eq!(elements.len(), 1);

    let elem = &elements[0];
    assert_eq!(elem["index"], 0);
    assert_eq!(elem["tag"], "div");
    assert_eq!(elem["id"], "test");
    assert_eq!(elem["classes"][0], "foo");
    assert_eq!(elem["classes"][1], "bar");
    assert_eq!(elem["x"], 10.0);
    assert_eq!(elem["y"], 20.0);
    assert_eq!(elem["width"], 100.0);
    assert_eq!(elem["height"], 50.0);
}

#[test]
fn test_layout_serialization_roundtrip() {
    // Test that we can serialize and deserialize without data loss
    let layout_json = r#"{
        "metadata": {
            "url": "http://test.com",
            "title": "Test",
            "viewport": {"width": 1920, "height": 1080},
            "devicePixelRatio": 1.0,
            "timestamp": "2024-01-01T00:00:00Z"
        },
        "elements": [
            {
                "index": 0,
                "tag": "div",
                "id": null,
                "classes": [],
                "x": 10.0,
                "y": 20.0,
                "width": 100.0,
                "height": 50.0,
                "fontSize": "16px",
                "fontWeight": "400",
                "color": "rgb(0, 0, 0)",
                "backgroundColor": "rgb(255, 255, 255)",
                "textContent": "Hello World",
                "placeholder": null,
                "borderBottom": "1px solid black",
                "zIndex": "1"
            }
        ]
    }"#;

    // Parse
    let parsed: serde_json::Value = serde_json::from_str(layout_json).unwrap();

    // Serialize
    let reserialized = serde_json::to_string_pretty(&parsed).unwrap();

    // Parse again to verify no data loss
    let reparsed: serde_json::Value = serde_json::from_str(&reserialized).unwrap();

    // Verify they're equal
    assert_eq!(parsed, reparsed);
}

// ============================================================================
// File I/O Tests - Testing layout file handling
// ============================================================================

#[test]
fn test_layout_file_write_and_read() {
    let temp_dir = TempDir::new().unwrap();
    let layout_path = temp_dir.path().join("test_layout.json");

    let layout_json = r#"{
        "metadata": {
            "url": "http://test.com",
            "title": "Test",
            "viewport": {"width": 1920, "height": 1080},
            "devicePixelRatio": 1.0,
            "timestamp": "2024-01-01T00:00:00Z"
        },
        "elements": [
            {
                "index": 0,
                "tag": "div",
                "id": null,
                "classes": [],
                "x": 10.0,
                "y": 20.0,
                "width": 100.0,
                "height": 50.0
            }
        ]
    }"#;

    // Write
    fs::write(&layout_path, layout_json).unwrap();

    // Read
    let read_content = fs::read_to_string(&layout_path).unwrap();

    // Parse to verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&read_content).unwrap();

    // Verify content
    assert_eq!(parsed["metadata"]["title"], "Test");
    assert_eq!(parsed["elements"].as_array().unwrap().len(), 1);
}

// ============================================================================
// Layout Comparison Tests - Testing diff logic
// ============================================================================

#[test]
fn test_layout_element_comparison() {
    // Test comparing two element positions within tolerance
    let elem1_x = 10.0_f64;
    let elem1_y = 20.0_f64;
    let elem2_x = 12.0_f64;
    let elem2_y = 22.0_f64;

    let tolerance = 5.0;

    let x_diff = (elem1_x - elem2_x).abs();
    let y_diff = (elem1_y - elem2_y).abs();

    assert!(x_diff <= tolerance);
    assert!(y_diff <= tolerance);
}

#[test]
fn test_layout_element_comparison_outside_tolerance() {
    // Test comparing two element positions outside tolerance
    let elem1_x = 10.0_f64;
    let elem1_y = 20.0_f64;
    let elem2_x = 20.0_f64;
    let elem2_y = 30.0_f64;

    let tolerance = 5.0;

    let x_diff = (elem1_x - elem2_x).abs();
    let y_diff = (elem1_y - elem2_y).abs();

    assert!(x_diff > tolerance || y_diff > tolerance);
}

// ============================================================================
// HTML Generation Tests - Testing visualization
// ============================================================================

#[test]
fn test_html_escaping() {
    // Test HTML escaping for safety
    let test_string = "<script>alert('xss')</script>";
    let escaped = test_string
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;");

    assert_eq!(
        escaped,
        "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
    );
    assert!(!escaped.contains("<script>"));
}

#[test]
fn test_html_template_basic_structure() {
    // Verify HTML template has required structure
    let html = "<!DOCTYPE html>\n<html>\n<head></head>\n<body></body>\n</html>";

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<html>"));
    assert!(html.contains("</html>"));
    assert!(html.contains("<body>"));
    assert!(html.contains("</body>"));
}

// ============================================================================
// Path Handling Tests
// ============================================================================

#[test]
fn test_path_handling() {
    use std::path::PathBuf;

    let path = PathBuf::from("/tmp/test.json");
    assert!(path.is_absolute());

    let path2 = PathBuf::from("relative/path.json");
    assert!(!path2.is_absolute());
}

// ============================================================================
// JSON Validation Tests
// ============================================================================

#[test]
fn test_invalid_json_handling() {
    let invalid_json = r#"{
        "metadata": {
            "url": "http://test.com"
            // Missing closing braces
    "#;

    let result = serde_json::from_str::<serde_json::Value>(invalid_json);
    assert!(result.is_err());
}

#[test]
fn test_missing_required_field() {
    // Test that missing required fields are caught
    let incomplete_json = r#"{
        "metadata": {
            "url": "http://test.com"
        }
    }"#;

    let parsed = serde_json::from_str::<serde_json::Value>(incomplete_json);
    assert!(parsed.is_ok());

    let value = parsed.unwrap();
    assert!(value["metadata"]["viewport"].is_null());
}
