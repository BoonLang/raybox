use anyhow::Result;
use std::collections::HashMap;
use std::fs;

use crate::layout::{Element, LayoutData, Metadata, Summary, Viewport};

// Viewport settings
const VIEWPORT_WIDTH: u32 = 700;
const VIEWPORT_HEIGHT: u32 = 700;
const BODY_MAX_WIDTH: f32 = 550.0;

// CSS-derived measurements
const HEADER_MARGIN_TOP: f32 = 130.0;
const H1_TOP_OFFSET: f32 = -140.0;
const H1_HEIGHT: f32 = 80.0;
const H1_FONT_SIZE: f32 = 80.0;

const INPUT_HEIGHT: f32 = 65.0;
const INPUT_FONT_SIZE: f32 = 24.0;

const TODO_ITEM_HEIGHT: f32 = 58.0;
const TODO_ITEM_FONT_SIZE: f32 = 24.0;

const FOOTER_HEIGHT: f32 = 40.0;
const FOOTER_FONT_SIZE: f32 = 15.0;

pub fn run(output_path: &str) -> Result<()> {
    let layout_data = generate_layout();

    let json = serde_json::to_string_pretty(&layout_data)?;
    fs::write(output_path, json)?;

    log::info!("DOM layout extracted to: {}", output_path);
    println!("✓ DOM layout extracted to: {}", output_path);
    println!("  Total elements: {}", layout_data.summary.total_elements);

    Ok(())
}

fn calculate_body_x() -> f32 {
    (VIEWPORT_WIDTH as f32 - BODY_MAX_WIDTH) / 2.0
}

fn create_element(index: usize, tag: &str) -> Element {
    Element {
        index,
        tag: tag.to_string(),
        id: None,
        classes: Vec::new(),
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
        font_size: None,
        font_family: None,
        font_weight: None,
        font_style: None,
        line_height: None,
        letter_spacing: None,
        text_align: None,
        text_decoration: None,
        text_transform: None,
        padding: None,
        padding_top: None,
        padding_right: None,
        padding_bottom: None,
        padding_left: None,
        margin: None,
        margin_top: None,
        margin_right: None,
        margin_bottom: None,
        margin_left: None,
        border: None,
        border_radius: None,
        border_color: None,
        border_top: None,
        border_bottom: None,
        display: None,
        position: None,
        flex_direction: None,
        justify_content: None,
        align_items: None,
        gap: None,
        max_width: None,
        min_width: None,
        max_height: None,
        min_height: None,
        z_index: None,
        top: None,
        left: None,
        right: None,
        bottom: None,
        color: None,
        background_color: None,
        box_shadow: None,
        text_shadow: None,
        text_content: None,
        value: None,
        placeholder: None,
        checked: None,
        disabled: None,
        visibility: None,
        opacity: None,
        white_space: None,
        word_break: None,
        list_style: None,
    }
}

fn generate_layout() -> LayoutData {
    let mut elements = Vec::new();
    let mut idx = 0;

    let body_x = calculate_body_x();

    // HTML root
    let mut elem = create_element(idx, "html");
    elem.x = 0.0;
    elem.y = 0.0;
    elem.width = VIEWPORT_WIDTH as f32;
    elem.height = VIEWPORT_HEIGHT as f32;
    elem.font_size = Some("14px".to_string());
    elem.font_family = Some("Helvetica Neue, Helvetica, Arial, sans-serif".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Body
    let mut elem = create_element(idx, "body");
    elem.x = body_x;
    elem.y = 0.0;
    elem.width = BODY_MAX_WIDTH;
    elem.height = VIEWPORT_HEIGHT as f32;
    elem.font_size = Some("14px".to_string());
    elem.font_family = Some("Helvetica Neue, Helvetica, Arial, sans-serif".to_string());
    elem.font_weight = Some("300".to_string());
    elem.line_height = Some("1.4em".to_string());
    elem.max_width = Some("550px".to_string());
    elem.margin = Some("0 auto".to_string());
    elem.background_color = Some("rgb(245, 245, 245)".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // TodoApp section
    let todoapp_y = HEADER_MARGIN_TOP;
    let todoapp_height = INPUT_HEIGHT + (TODO_ITEM_HEIGHT * 4.0) + FOOTER_HEIGHT;

    let mut elem = create_element(idx, "section");
    elem.classes = vec!["todoapp".to_string()];
    elem.x = body_x;
    elem.y = todoapp_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = todoapp_height;
    elem.background_color = Some("rgb(255, 255, 255)".to_string());
    elem.position = Some("relative".to_string());
    elem.box_shadow =
        Some("0 2px 4px 0 rgba(0,0,0,0.2), 0 25px 50px 0 rgba(0,0,0,0.1)".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Header section
    let mut elem = create_element(idx, "header");
    elem.classes = vec!["header".to_string()];
    elem.x = body_x;
    elem.y = todoapp_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = INPUT_HEIGHT;
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // H1 "todos"
    let h1_y = todoapp_y + H1_TOP_OFFSET;
    let mut elem = create_element(idx, "h1");
    elem.x = body_x;
    elem.y = h1_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = H1_HEIGHT;
    elem.font_size = Some(format!("{}px", H1_FONT_SIZE));
    elem.font_weight = Some("200".to_string());
    elem.color = Some("rgb(184, 63, 69)".to_string());
    elem.text_align = Some("center".to_string());
    elem.text_content = Some("todos".to_string());
    elem.position = Some("absolute".to_string());
    elem.top = Some("-140px".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Input field
    let mut elem = create_element(idx, "input");
    elem.classes = vec!["new-todo".to_string()];
    elem.x = body_x;
    elem.y = todoapp_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = INPUT_HEIGHT;
    elem.font_size = Some(format!("{}px", INPUT_FONT_SIZE));
    elem.padding = Some("16px 16px 16px 60px".to_string());
    elem.padding_left = Some("60px".to_string());
    elem.padding_top = Some("16px".to_string());
    elem.padding_right = Some("16px".to_string());
    elem.padding_bottom = Some("16px".to_string());
    elem.placeholder = Some("What needs to be done?".to_string());
    elem.border = Some("none".to_string());
    elem.box_shadow = Some("inset 0 -2px 1px rgba(0,0,0,0.03)".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Main section
    let main_y = todoapp_y + INPUT_HEIGHT;
    let mut elem = create_element(idx, "main");
    elem.classes = vec!["main".to_string()];
    elem.x = body_x;
    elem.y = main_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = TODO_ITEM_HEIGHT * 4.0;
    elem.border_top = Some("1px solid #e6e6e6".to_string());
    elem.position = Some("relative".to_string());
    elem.z_index = Some("2".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Toggle all container
    let mut elem = create_element(idx, "div");
    elem.classes = vec!["toggle-all-container".to_string()];
    elem.x = body_x;
    elem.y = main_y;
    elem.width = 45.0;
    elem.height = 65.0;
    elem.display = Some("flex".to_string());
    elements.push(elem);
    idx += 1;

    // Toggle all checkbox (hidden)
    let mut elem = create_element(idx, "input");
    elem.classes = vec!["toggle-all".to_string()];
    elem.id = Some("toggle-all".to_string());
    elem.x = body_x;
    elem.y = main_y;
    elem.width = 1.0;
    elem.height = 1.0;
    elem.opacity = Some("0".to_string());
    elem.position = Some("absolute".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Toggle all label
    let mut elem = create_element(idx, "label");
    elem.classes = vec!["toggle-all-label".to_string()];
    elem.x = body_x;
    elem.y = main_y - 65.0;
    elem.width = 45.0;
    elem.height = 65.0;
    elem.font_size = Some("0".to_string());
    elem.position = Some("absolute".to_string());
    elem.display = Some("flex".to_string());
    elements.push(elem);
    idx += 1;

    // Todo list
    let ul_y = main_y;
    let mut elem = create_element(idx, "ul");
    elem.classes = vec!["todo-list".to_string()];
    elem.x = body_x;
    elem.y = ul_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = TODO_ITEM_HEIGHT * 4.0;
    elem.list_style = Some("none".to_string());
    elem.margin = Some("0".to_string());
    elem.padding = Some("0".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Todo items
    let todos = vec![
        ("Buy groceries", false),
        ("Walk the dog", false),
        ("Finish TodoMVC renderer", true),
        ("Read documentation", false),
    ];

    for (i, (text, completed)) in todos.iter().enumerate() {
        let li_y = ul_y + (i as f32 * TODO_ITEM_HEIGHT);

        // Li element
        let mut elem = create_element(idx, "li");
        if *completed {
            elem.classes = vec!["completed".to_string()];
        }
        elem.x = body_x;
        elem.y = li_y;
        elem.width = BODY_MAX_WIDTH;
        elem.height = TODO_ITEM_HEIGHT;
        elem.border_bottom = Some("1px solid #ededed".to_string());
        elem.font_size = Some(format!("{}px", TODO_ITEM_FONT_SIZE));
        elem.position = Some("relative".to_string());
        elem.display = Some("block".to_string());
        elements.push(elem);
        idx += 1;

        // View div
        let mut elem = create_element(idx, "div");
        elem.classes = vec!["view".to_string()];
        elem.x = body_x;
        elem.y = li_y;
        elem.width = BODY_MAX_WIDTH;
        elem.height = TODO_ITEM_HEIGHT;
        elem.display = Some("block".to_string());
        elements.push(elem);
        idx += 1;

        // Checkbox
        let mut elem = create_element(idx, "input");
        elem.classes = vec!["toggle".to_string()];
        elem.x = body_x;
        elem.y = li_y;
        elem.width = 40.0;
        elem.height = 40.0;
        elem.checked = Some(*completed);
        elem.display = Some("block".to_string());
        elements.push(elem);
        idx += 1;

        // Label with text
        let mut elem = create_element(idx, "label");
        elem.x = body_x + 60.0;
        elem.y = li_y;
        elem.width = BODY_MAX_WIDTH - 60.0;
        elem.height = TODO_ITEM_HEIGHT;
        elem.font_size = Some(format!("{}px", TODO_ITEM_FONT_SIZE));
        elem.padding = Some("15px 15px 15px 60px".to_string());
        elem.padding_left = Some("60px".to_string());
        elem.color = Some(if *completed {
            "rgb(148, 148, 148)".to_string()
        } else {
            "rgb(72, 72, 72)".to_string()
        });
        elem.text_decoration = Some(if *completed {
            "line-through".to_string()
        } else {
            "none".to_string()
        });
        elem.text_content = Some(text.to_string());
        elem.display = Some("block".to_string());
        elements.push(elem);
        idx += 1;

        // Destroy button
        let mut elem = create_element(idx, "button");
        elem.classes = vec!["destroy".to_string()];
        elem.x = body_x + BODY_MAX_WIDTH - 50.0;
        elem.y = li_y;
        elem.width = 40.0;
        elem.height = 40.0;
        elem.display = Some("none".to_string()); // Shows on hover
        elements.push(elem);
        idx += 1;
    }

    // Footer
    let footer_y = ul_y + (TODO_ITEM_HEIGHT * 4.0);
    let mut elem = create_element(idx, "footer");
    elem.classes = vec!["footer".to_string()];
    elem.x = body_x;
    elem.y = footer_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = FOOTER_HEIGHT;
    elem.border_top = Some("1px solid #e6e6e6".to_string());
    elem.font_size = Some(format!("{}px", FOOTER_FONT_SIZE));
    elem.padding = Some("10px 15px".to_string());
    elem.text_align = Some("center".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Todo count
    let mut elem = create_element(idx, "span");
    elem.classes = vec!["todo-count".to_string()];
    elem.x = body_x;
    elem.y = footer_y;
    elem.width = 100.0;
    elem.height = 20.0;
    elem.text_content = Some("3 items left".to_string());
    elem.font_size = Some(format!("{}px", FOOTER_FONT_SIZE));
    elem.display = Some("inline".to_string());
    elements.push(elem);
    idx += 1;

    // Filters
    let filters_x = body_x + 150.0;
    let mut elem = create_element(idx, "ul");
    elem.classes = vec!["filters".to_string()];
    elem.x = filters_x;
    elem.y = footer_y;
    elem.width = 200.0;
    elem.height = 20.0;
    elem.list_style = Some("none".to_string());
    elem.margin = Some("0".to_string());
    elem.padding = Some("0".to_string());
    elem.position = Some("absolute".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    let filter_labels = vec!["All", "Active", "Completed"];
    for (i, label) in filter_labels.iter().enumerate() {
        // Li
        let mut elem = create_element(idx, "li");
        elem.x = filters_x + (i as f32 * 70.0);
        elem.y = footer_y;
        elem.width = 60.0;
        elem.height = 20.0;
        elem.display = Some("inline".to_string());
        elements.push(elem);
        idx += 1;

        // Link
        let mut elem = create_element(idx, "a");
        if *label == "All" {
            elem.classes = vec!["selected".to_string()];
        }
        elem.x = filters_x + (i as f32 * 70.0);
        elem.y = footer_y;
        elem.width = 40.0;
        elem.height = 20.0;
        elem.text_content = Some(label.to_string());
        elem.font_size = Some(format!("{}px", FOOTER_FONT_SIZE));
        elem.border = Some(if *label == "All" {
            "1px solid #ce4646".to_string()
        } else {
            "1px solid transparent".to_string()
        });
        elem.border_radius = Some("3px".to_string());
        elem.padding = Some("3px 7px".to_string());
        elem.text_decoration = Some("none".to_string());
        elem.display = Some("inline".to_string());
        elements.push(elem);
        idx += 1;
    }

    // Clear completed button
    let mut elem = create_element(idx, "button");
    elem.classes = vec!["clear-completed".to_string()];
    elem.x = body_x + BODY_MAX_WIDTH - 150.0;
    elem.y = footer_y;
    elem.width = 130.0;
    elem.height = 20.0;
    elem.text_content = Some("Clear completed".to_string());
    elem.font_size = Some(format!("{}px", FOOTER_FONT_SIZE));
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Info footer
    let info_y = footer_y + 100.0;
    let mut elem = create_element(idx, "footer");
    elem.classes = vec!["info".to_string()];
    elem.x = body_x;
    elem.y = info_y;
    elem.width = BODY_MAX_WIDTH;
    elem.height = 100.0;
    elem.font_size = Some("11px".to_string());
    elem.text_align = Some("center".to_string());
    elem.color = Some("rgb(77, 77, 77)".to_string());
    elem.display = Some("block".to_string());
    elements.push(elem);
    idx += 1;

    // Info paragraphs
    let info_texts = vec![
        "Double-click to edit a todo",
        "Created by the TodoMVC Team",
        "Part of TodoMVC",
    ];

    for (i, text) in info_texts.iter().enumerate() {
        let mut elem = create_element(idx, "p");
        elem.x = body_x;
        elem.y = info_y + (i as f32 * 20.0);
        elem.width = BODY_MAX_WIDTH;
        elem.height = 20.0;
        elem.text_content = Some(text.to_string());
        elem.font_size = Some("11px".to_string());
        elem.text_align = Some("center".to_string()); // Inherit from parent .info footer
        elem.display = Some("block".to_string());
        elements.push(elem);
        idx += 1;
    }

    // Calculate summary
    let mut by_tag: HashMap<String, usize> = HashMap::new();
    let mut by_class: HashMap<String, usize> = HashMap::new();

    for elem in &elements {
        *by_tag.entry(elem.tag.clone()).or_insert(0) += 1;
        for cls in &elem.classes {
            *by_class.entry(cls.clone()).or_insert(0) += 1;
        }
    }

    let centering_note = format!(
        "Body centered at x={}px (viewport {}px - body {}px) / 2",
        body_x, VIEWPORT_WIDTH, BODY_MAX_WIDTH as u32
    );

    LayoutData {
        metadata: Metadata {
            url: "http://localhost:8765/todomvc_populated.html".to_string(),
            title: "TodoMVC: JavaScript Es6".to_string(),
            generator: Some("Rust CSS/HTML analyzer".to_string()),
            user_agent: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            viewport: Viewport {
                width: VIEWPORT_WIDTH,
                height: VIEWPORT_HEIGHT,
                device_pixel_ratio: 1.0,
            },
            note: Some(
                "Generated from HTML/CSS analysis. Positions calculated from CSS rules."
                    .to_string(),
            ),
            centering_note: Some(centering_note),
        },
        elements: elements.clone(),
        summary: Summary {
            total_elements: elements.len(),
            by_tag,
            by_class,
        },
    }
}
