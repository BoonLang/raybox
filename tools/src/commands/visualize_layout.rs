use anyhow::Result;
use std::fs;

use crate::layout::LayoutData;

const HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>Layout Visualization - {title}</title>
  <style>
    body {{
      margin: 0;
      padding: 20px;
      font-family: 'Helvetica Neue', Helvetica, Arial, sans-serif;
      background: #f0f0f0;
    }}

    .container {{
      max-width: 1920px;
      margin: 0 auto;
      background: white;
      box-shadow: 0 2px 10px rgba(0,0,0,0.1);
    }}

    .controls {{
      padding: 20px;
      background: #fff;
      border-bottom: 1px solid #ddd;
      position: sticky;
      top: 0;
      z-index: 1000;
    }}

    .controls label {{
      margin-right: 20px;
      cursor: pointer;
    }}

    .controls input[type="checkbox"] {{
      margin-right: 5px;
    }}

    .viewport {{
      position: relative;
      width: {viewport_width}px;
      height: {viewport_height}px;
      background: rgb(245, 245, 245);
      margin: 20px auto;
    }}

    .element {{
      position: absolute;
      border: 1px solid rgba(0, 0, 0, 0.3);
      box-sizing: border-box;
      transition: all 0.2s;
      cursor: pointer;
    }}

    .element:hover {{
      border: 2px solid #4CAF50;
      z-index: 999 !important;
    }}

    .element.show-labels::after {{
      content: attr(data-label);
      position: absolute;
      top: 0;
      left: 0;
      background: rgba(0, 0, 0, 0.8);
      color: white;
      padding: 2px 5px;
      font-size: 10px;
      white-space: nowrap;
      pointer-events: none;
      z-index: 1;
    }}

    .element.show-content {{
      overflow: hidden;
      font-family: inherit;
    }}

    .element.show-content .content {{
      display: block;
      padding: 2px;
      font-size: 12px;
      color: #333;
      overflow: hidden;
      text-overflow: ellipsis;
    }}

    .element .content {{
      display: none;
    }}

    /* Color by tag */
    .element[data-tag="html"] {{ background: rgba(255, 0, 0, 0.05); }}
    .element[data-tag="body"] {{ background: rgba(0, 255, 0, 0.05); }}
    .element[data-tag="section"] {{ background: rgba(0, 0, 255, 0.05); }}
    .element[data-tag="header"] {{ background: rgba(255, 255, 0, 0.05); }}
    .element[data-tag="h1"] {{ background: rgba(255, 0, 255, 0.1); }}
    .element[data-tag="input"] {{ background: rgba(0, 255, 255, 0.1); }}
    .element[data-tag="ul"] {{ background: rgba(128, 128, 0, 0.05); }}
    .element[data-tag="li"] {{ background: rgba(0, 128, 128, 0.05); }}
    .element[data-tag="label"] {{ background: rgba(128, 0, 128, 0.05); }}
    .element[data-tag="button"] {{ background: rgba(255, 128, 0, 0.1); }}
    .element[data-tag="footer"] {{ background: rgba(128, 128, 128, 0.05); }}

    .info {{
      position: fixed;
      bottom: 20px;
      right: 20px;
      background: white;
      padding: 15px;
      border-radius: 5px;
      box-shadow: 0 2px 10px rgba(0,0,0,0.2);
      max-width: 400px;
      display: none;
    }}

    .info.visible {{
      display: block;
    }}

    .info h3 {{
      margin: 0 0 10px 0;
      font-size: 14px;
    }}

    .info table {{
      width: 100%;
      font-size: 12px;
      border-collapse: collapse;
    }}

    .info td {{
      padding: 3px 5px;
      border-bottom: 1px solid #eee;
    }}

    .info td:first-child {{
      font-weight: bold;
      width: 40%;
    }}

    .legend {{
      padding: 20px;
      background: #f9f9f9;
      margin-top: 20px;
    }}

    .legend h3 {{
      margin: 0 0 10px 0;
    }}

    .legend-item {{
      display: inline-block;
      margin-right: 15px;
      margin-bottom: 5px;
    }}

    .legend-box {{
      display: inline-block;
      width: 20px;
      height: 12px;
      border: 1px solid rgba(0, 0, 0, 0.3);
      margin-right: 5px;
      vertical-align: middle;
    }}
  </style>
</head>
<body>
  <div class="container">
    <div class="controls">
      <label><input type="checkbox" id="show-labels"> Show labels</label>
      <label><input type="checkbox" id="show-content"> Show text content</label>
      <label><input type="checkbox" id="hide-large" checked> Hide html/body</label>
      <span style="margin-left: 20px;">Total elements: <strong>{total_elements}</strong></span>
    </div>

    <div class="viewport">
      {elements_html}
    </div>

    <div class="legend">
      <h3>Element Types</h3>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(255, 0, 255, 0.1);"></span>
        <span>h1</span>
      </div>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(0, 255, 255, 0.1);"></span>
        <span>input</span>
      </div>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(128, 0, 128, 0.05);"></span>
        <span>label</span>
      </div>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(0, 128, 128, 0.05);"></span>
        <span>li</span>
      </div>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(255, 128, 0, 0.1);"></span>
        <span>button</span>
      </div>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(0, 0, 255, 0.05);"></span>
        <span>section</span>
      </div>
      <div class="legend-item">
        <span class="legend-box" style="background: rgba(128, 128, 128, 0.05);"></span>
        <span>footer</span>
      </div>
    </div>
  </div>

  <div class="info" id="info-panel">
    <h3>Element Info</h3>
    <div id="info-content"></div>
  </div>

  <script>
    const elements = {elements_json};

    // Controls
    document.getElementById('show-labels').addEventListener('change', (e) => {{
      document.querySelectorAll('.element').forEach(el => {{
        el.classList.toggle('show-labels', e.target.checked);
      }});
    }});

    document.getElementById('show-content').addEventListener('change', (e) => {{
      document.querySelectorAll('.element').forEach(el => {{
        el.classList.toggle('show-content', e.target.checked);
      }});
    }});

    document.getElementById('hide-large').addEventListener('change', (e) => {{
      document.querySelectorAll('.element[data-tag="html"], .element[data-tag="body"]').forEach(el => {{
        el.style.display = e.target.checked ? 'none' : 'block';
      }});
    }});

    // Element click handler
    document.querySelectorAll('.element').forEach(el => {{
      el.addEventListener('click', (e) => {{
        e.stopPropagation();
        const index = parseInt(el.dataset.index);
        const elem = elements.find(e => e.index === index);

        if (elem) {{
          showInfo(elem);
        }}
      }});
    }});

    // Click outside to hide info
    document.addEventListener('click', () => {{
      document.getElementById('info-panel').classList.remove('visible');
    }});

    function showInfo(elem) {{
      const infoContent = document.getElementById('info-content');
      const table = document.createElement('table');

      const props = [
        ['Index', elem.index],
        ['Tag', elem.tag],
        ['ID', elem.id || 'none'],
        ['Classes', elem.classes.join(', ') || 'none'],
        ['Position', `x=${{elem.x}}, y=${{elem.y}}`],
        ['Size', `${{elem.width}} × ${{elem.height}}px`],
        ['Font Size', elem.fontSize || 'N/A'],
        ['Font Weight', elem.fontWeight || 'N/A'],
        ['Color', elem.color || 'N/A'],
        ['Background', elem.backgroundColor || 'N/A'],
        ['Content', elem.textContent || elem.placeholder || 'none'],
      ];

      props.forEach(([key, value]) => {{
        const row = table.insertRow();
        row.insertCell(0).textContent = key;
        row.insertCell(1).textContent = value;
      }});

      infoContent.innerHTML = '';
      infoContent.appendChild(table);
      document.getElementById('info-panel').classList.add('visible');
    }}
  </script>
</body>
</html>
"#;

pub fn run(input: &str, output: &str) -> Result<()> {
    log::info!("Visualizing layout: {} -> {}", input, output);

    // Load layout data
    let content = fs::read_to_string(input)?;
    let layout: LayoutData = serde_json::from_str(&content)?;

    // Generate HTML
    let html = generate_html(&layout)?;

    // Write output
    fs::write(output, html)?;

    println!("✓ Layout visualization generated: {}", output);
    println!("  Total elements: {}", layout.elements.len());
    println!("  Open in browser: file://{}", std::fs::canonicalize(output)?.display());

    Ok(())
}

fn generate_html(layout: &LayoutData) -> Result<String> {
    let viewport_width = layout.metadata.viewport.width;
    let viewport_height = layout.metadata.viewport.height;

    // Generate element HTML
    let mut elements_html = Vec::new();

    for elem in &layout.elements {
        let x = elem.x;
        let y = elem.y;
        let width = elem.width;
        let height = elem.height;

        let tag = &elem.tag;
        let index = elem.index;

        // Create label
        let mut label_parts = vec![format!("#{}", index), tag.clone()];
        if let Some(id) = &elem.id {
            label_parts.push(format!("#{}", id));
        }
        if !elem.classes.is_empty() {
            label_parts.push(format!(".{}", elem.classes.join(".")));
        }
        let label = label_parts.join(" ");

        // Content
        let content = elem
            .text_content
            .as_ref()
            .or(elem.placeholder.as_ref())
            .map(|s| {
                if s.len() > 50 {
                    format!("{}...", &s[..47])
                } else {
                    s.clone()
                }
            })
            .unwrap_or_default();

        // Style
        let style = format!(
            "left: {}px; top: {}px; width: {}px; height: {}px;",
            x, y, width, height
        );

        // Z-index if present
        let style = if let Some(z) = &elem.z_index {
            format!("{} z-index: {};", style, z)
        } else {
            style
        };

        // Escape HTML in label and content
        let label = html_escape(&label);
        let content = html_escape(&content);

        // Generate HTML
        let html = if !content.is_empty() {
            format!(
                r#"<div class="element" data-tag="{}" data-index="{}" data-label="{}" style="{}"><span class="content">{}</span></div>"#,
                tag, index, label, style, content
            )
        } else {
            format!(
                r#"<div class="element" data-tag="{}" data-index="{}" data-label="{}" style="{}"></div>"#,
                tag, index, label, style
            )
        };

        elements_html.push(html);
    }

    // Serialize elements as JSON for JavaScript
    let elements_json = serde_json::to_string(&layout.elements)?;

    // Fill template
    let html = HTML_TEMPLATE
        .replace("{title}", &layout.metadata.title)
        .replace("{viewport_width}", &viewport_width.to_string())
        .replace("{viewport_height}", &viewport_height.to_string())
        .replace("{total_elements}", &layout.elements.len().to_string())
        .replace("{elements_html}", &elements_html.join("\n      "))
        .replace("{elements_json}", &elements_json);

    Ok(html)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
