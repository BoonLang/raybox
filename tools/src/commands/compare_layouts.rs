use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::layout::{Element, LayoutData};

const MAX_ERROR_THRESHOLD: f32 = 5.0; // Success threshold in pixels

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ElementComparison {
    reference: Element,
    actual: Element,
    errors: HashMap<String, f32>,
    passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VisualDiff {
    metadata: DiffMetadata,
    elements: Vec<ElementDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiffMetadata {
    reference_file: String,
    actual_file: String,
    comparison_timestamp: String,
    max_error_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ElementDiff {
    index: usize,
    tag: String,
    classes: Vec<String>,
    reference: Position,
    actual: Position,
    errors: HashMap<String, f32>,
    passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Position {
    x: Option<f32>,
    y: Option<f32>,
    width: Option<f32>,
    height: Option<f32>,
}

pub fn run(reference_path: &str, actual_path: &str, diff_output: Option<&str>) -> Result<()> {
    // Load layout data
    let reference: LayoutData = load_layout(reference_path).context(format!(
        "Failed to load reference layout: {}",
        reference_path
    ))?;

    let actual: LayoutData = load_layout(actual_path)
        .context(format!("Failed to load actual layout: {}", actual_path))?;

    // Perform comparison
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let comparisons = compare_by_index(&reference, &actual, &mut errors, &mut warnings)?;

    // Generate report
    let report = generate_report(&reference, &actual, &comparisons, &errors, &warnings);
    println!("{}", report);

    // Export visual diff if requested
    if let Some(output_path) = diff_output {
        export_visual_diff(reference_path, actual_path, &comparisons, output_path)?;
        println!("\nVisual diff exported to: {}", output_path);
    }

    // Return error if comparison failed
    let passed_count = comparisons.values().filter(|c| c.passed).count();
    let total_count = comparisons.len();

    if passed_count < total_count || !errors.is_empty() {
        anyhow::bail!(
            "Comparison failed: {}/{} elements passed",
            passed_count,
            total_count
        );
    }

    Ok(())
}

fn load_layout(path: &str) -> Result<LayoutData> {
    let content = fs::read_to_string(path)?;
    let layout: LayoutData = serde_json::from_str(&content)?;
    Ok(layout)
}

fn calculate_position_error(ref_elem: &Element, actual_elem: &Element) -> HashMap<String, f32> {
    let mut errors = HashMap::new();

    // Position errors
    errors.insert("x".to_string(), (ref_elem.x - actual_elem.x).abs());
    errors.insert("y".to_string(), (ref_elem.y - actual_elem.y).abs());

    // Size errors
    errors.insert(
        "width".to_string(),
        (ref_elem.width - actual_elem.width).abs(),
    );
    errors.insert(
        "height".to_string(),
        (ref_elem.height - actual_elem.height).abs(),
    );

    // Calculate total positional error (Euclidean distance)
    let x_err = errors["x"];
    let y_err = errors["y"];
    let position_distance = (x_err * x_err + y_err * y_err).sqrt();
    errors.insert("position_distance".to_string(), position_distance);

    errors
}

fn compare_by_index(
    reference: &LayoutData,
    actual: &LayoutData,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Result<HashMap<usize, ElementComparison>> {
    let mut results = HashMap::new();

    // Create index maps
    let ref_elements: HashMap<usize, &Element> =
        reference.elements.iter().map(|e| (e.index, e)).collect();

    let actual_elements: HashMap<usize, &Element> =
        actual.elements.iter().map(|e| (e.index, e)).collect();

    // Check all reference elements
    for (&idx, ref_elem) in &ref_elements {
        if !actual_elements.contains_key(&idx) {
            errors.push(format!(
                "Missing element at index {}: {}.{:?}",
                idx, ref_elem.tag, ref_elem.classes
            ));
            continue;
        }

        let actual_elem = actual_elements[&idx];

        // Check tag matches
        if ref_elem.tag != actual_elem.tag {
            errors.push(format!(
                "Tag mismatch at index {}: expected {}, got {}",
                idx, ref_elem.tag, actual_elem.tag
            ));
            continue;
        }

        // Calculate errors
        let position_errors = calculate_position_error(ref_elem, actual_elem);
        let passed = position_errors.values().all(|&v| v <= MAX_ERROR_THRESHOLD);

        results.insert(
            idx,
            ElementComparison {
                reference: (*ref_elem).clone(),
                actual: (*actual_elem).clone(),
                errors: position_errors,
                passed,
            },
        );
    }

    // Check for extra elements in actual
    for &idx in actual_elements.keys() {
        if !ref_elements.contains_key(&idx) {
            let elem = actual_elements[&idx];
            warnings.push(format!("Extra element at index {}: {}", idx, elem.tag));
        }
    }

    Ok(results)
}

fn generate_report(
    reference: &LayoutData,
    actual: &LayoutData,
    comparisons: &HashMap<usize, ElementComparison>,
    errors: &[String],
    warnings: &[String],
) -> String {
    let mut lines = Vec::new();

    lines.push("=".repeat(80));
    lines.push("LAYOUT COMPARISON REPORT".to_string());
    lines.push("=".repeat(80));
    lines.push(String::new());

    // Summary
    lines.push(format!("Reference: {} elements", reference.elements.len()));
    lines.push(format!("Actual: {} elements", actual.elements.len()));
    lines.push(format!(
        "Success threshold: ≤{}px error",
        MAX_ERROR_THRESHOLD
    ));
    lines.push(String::new());

    // Element-by-element comparison
    lines.push("=".repeat(80));
    lines.push("ELEMENT-BY-ELEMENT COMPARISON".to_string());
    lines.push("=".repeat(80));
    lines.push(String::new());

    let passed_count = comparisons.values().filter(|c| c.passed).count();
    let total_count = comparisons.len();

    lines.push(format!("Passed: {}/{} elements", passed_count, total_count));
    lines.push(String::new());

    // Show failures
    let mut failures: Vec<_> = comparisons.iter().filter(|(_, c)| !c.passed).collect();
    failures.sort_by_key(|(idx, _)| *idx);

    if !failures.is_empty() {
        lines.push("FAILURES (errors > 5px):".to_string());
        lines.push("-".repeat(80));

        for (idx, comparison) in failures {
            let ref_elem = &comparison.reference;
            let actual_elem = &comparison.actual;
            let errs = &comparison.errors;

            lines.push(format!("Index {}: {}", idx, ref_elem.tag));

            if !ref_elem.classes.is_empty() {
                lines.push(format!("  Classes: {}", ref_elem.classes.join(", ")));
            }

            if let Some(id) = &ref_elem.id {
                lines.push(format!("  ID: {}", id));
            }

            lines.push("  Position error:".to_string());
            lines.push(format!(
                "    X: {:.2}px (ref={:.1}, actual={:.1})",
                errs.get("x").unwrap_or(&0.0),
                ref_elem.x,
                actual_elem.x
            ));
            lines.push(format!(
                "    Y: {:.2}px (ref={:.1}, actual={:.1})",
                errs.get("y").unwrap_or(&0.0),
                ref_elem.y,
                actual_elem.y
            ));

            if let Some(dist) = errs.get("position_distance") {
                lines.push(format!("    Total distance: {:.2}px", dist));
            }

            lines.push("  Size error:".to_string());
            lines.push(format!(
                "    Width: {:.2}px (ref={:.1}, actual={:.1})",
                errs.get("width").unwrap_or(&0.0),
                ref_elem.width,
                actual_elem.width
            ));
            lines.push(format!(
                "    Height: {:.2}px (ref={:.1}, actual={:.1})",
                errs.get("height").unwrap_or(&0.0),
                ref_elem.height,
                actual_elem.height
            ));

            lines.push(String::new());
        }
    } else {
        lines.push("✓ All elements within tolerance!".to_string());
        lines.push(String::new());
    }

    // Key elements check
    lines.push("=".repeat(80));
    lines.push("KEY ELEMENTS".to_string());
    lines.push("=".repeat(80));
    lines.push(String::new());

    let key_elements = [
        ("h1", None, "h1_title"),
        ("input", Some("new-todo"), "input_field"),
        ("ul", Some("todo-list"), "todo_list"),
        ("footer", Some("footer"), "footer"),
    ];

    for (tag, class, name) in &key_elements {
        let ref_elems: Vec<_> = reference
            .elements
            .iter()
            .filter(|e| {
                e.tag == *tag
                    && (class.is_none() || e.classes.contains(&class.unwrap().to_string()))
            })
            .collect();

        if ref_elems.is_empty() {
            continue;
        }

        let ref_elem = ref_elems[0];
        if let Some(comparison) = comparisons.get(&ref_elem.index) {
            let status = if comparison.passed {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            lines.push(format!("{} {}:", status, name));

            if let Some(dist) = comparison.errors.get("position_distance") {
                lines.push(format!("  Position error: {:.2}px", dist));
            }

            if let Some(x_err) = comparison.errors.get("x") {
                lines.push(format!("    X: {:.2}px", x_err));
            }

            if let Some(y_err) = comparison.errors.get("y") {
                lines.push(format!("    Y: {:.2}px", y_err));
            }

            lines.push(String::new());
        }
    }

    // Errors and warnings
    if !errors.is_empty() {
        lines.push("=".repeat(80));
        lines.push("ERRORS".to_string());
        lines.push("=".repeat(80));
        for error in errors {
            lines.push(format!("✗ {}", error));
        }
        lines.push(String::new());
    }

    if !warnings.is_empty() {
        lines.push("=".repeat(80));
        lines.push("WARNINGS".to_string());
        lines.push("=".repeat(80));
        for warning in warnings {
            lines.push(format!("⚠ {}", warning));
        }
        lines.push(String::new());
    }

    // Final verdict
    lines.push("=".repeat(80));
    lines.push("VERDICT".to_string());
    lines.push("=".repeat(80));

    if passed_count == total_count && errors.is_empty() {
        lines.push("✓ SUCCESS: All elements within 5px tolerance!".to_string());
    } else {
        lines.push(format!(
            "✗ FAILED: {} elements outside tolerance",
            total_count - passed_count
        ));
        lines.push(format!("  Passed: {}/{}", passed_count, total_count));
    }

    lines.push(String::new());

    lines.join("\n")
}

fn export_visual_diff(
    reference_path: &str,
    actual_path: &str,
    comparisons: &HashMap<usize, ElementComparison>,
    output_path: &str,
) -> Result<()> {
    let mut elements = Vec::new();

    for (idx, comparison) in comparisons {
        let ref_elem = &comparison.reference;
        let actual_elem = &comparison.actual;

        elements.push(ElementDiff {
            index: *idx,
            tag: ref_elem.tag.clone(),
            classes: ref_elem.classes.clone(),
            reference: Position {
                x: Some(ref_elem.x),
                y: Some(ref_elem.y),
                width: Some(ref_elem.width),
                height: Some(ref_elem.height),
            },
            actual: Position {
                x: Some(actual_elem.x),
                y: Some(actual_elem.y),
                width: Some(actual_elem.width),
                height: Some(actual_elem.height),
            },
            errors: comparison.errors.clone(),
            passed: comparison.passed,
        });
    }

    elements.sort_by_key(|e| e.index);

    let diff = VisualDiff {
        metadata: DiffMetadata {
            reference_file: reference_path.to_string(),
            actual_file: actual_path.to_string(),
            comparison_timestamp: chrono::Utc::now().to_rfc3339(),
            max_error_threshold: MAX_ERROR_THRESHOLD,
        },
        elements,
    };

    let json = serde_json::to_string_pretty(&diff)?;
    fs::write(output_path, json)?;

    Ok(())
}
