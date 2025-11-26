use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{routing::get_service, Router};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::{EventLoadEventFired, NavigateParams};
use chromiumoxide::cdp::js_protocol::runtime::EvaluateParams;
use chromiumoxide::Page;
use futures::StreamExt;
use tower_http::services::ServeDir;

use crate::layout_precise::{LayoutCapture, Metadata, Node, Rect, Viewport};

/// Options for a single capture run.
pub struct CaptureOptions<'a> {
    pub target_url: &'a str,
    pub out_path: &'a Path,
    pub viewport: Viewport,
    pub headed: bool,
    pub chrome_path: Option<&'a str>,
    pub is_reference: bool,
}

/// Capture reference layout using JS bounding boxes (fail-fast).
pub async fn run_capture_reference(
    file: &Path,
    out: &Path,
    headed: bool,
    chrome_path: Option<&str>,
    _layout_json: Option<&str>,
) -> Result<LayoutCapture> {
    let dir = file.parent().unwrap_or_else(|| Path::new("."));
    let (addr, shutdown) = start_static_server(dir).await?;
    let url = format!("http://{}:{}", addr.ip(), addr.port());
    let url = format!("{}/{}", url, file.file_name().unwrap().to_string_lossy());

    let capture = capture_layout(CaptureOptions {
        target_url: &url,
        out_path: out,
        viewport: Viewport {
            w: 700,
            h: 700,
            dpr: 1.0,
        },
        headed,
        chrome_path,
        is_reference: true,
    })
    .await?;

    let _ = shutdown.send(());
    Ok(capture)
}

/// Capture renderer layout (requires renderer already served, e.g., wasm-start).
pub async fn run_capture_renderer(
    url: &str,
    out: &Path,
    headed: bool,
    chrome_path: Option<&str>,
) -> Result<LayoutCapture> {
    capture_layout(CaptureOptions {
        target_url: url,
        out_path: out,
        viewport: Viewport {
            w: 700,
            h: 700,
            dpr: 1.0,
        },
        headed,
        chrome_path,
        is_reference: false,
    })
    .await
}

/// Diff two captures and print a git-like summary.
pub fn run_diff_layouts(a: &Path, b: &Path, threshold: f64) -> Result<()> {
    let left: LayoutCapture = serde_json::from_reader(std::fs::File::open(a)?)?;
    let right: LayoutCapture = serde_json::from_reader(std::fs::File::open(b)?)?;

    let mut diffs = Vec::new();

    // Index by id for simplicity (since renderer uses elem- ids, reference uses paths)
    let mut right_map: HashMap<&str, &Node> = HashMap::new();
    for n in &right.nodes {
        right_map.insert(n.id.as_str(), n);
    }

    for ln in &left.nodes {
        if let Some(rn) = right_map.get(ln.id.as_str()) {
            match (ln.box_model, rn.box_model) {
                (Some(lb), Some(rb)) => {
                    let dx = (lb.x - rb.x).abs();
                    let dy = (lb.y - rb.y).abs();
                    let dw = (lb.w - rb.w).abs();
                    let dh = (lb.h - rb.h).abs();
                    if dx > threshold || dy > threshold || dw > threshold || dh > threshold {
                        diffs.push((ln.id.clone(), dx, dy, dw, dh, lb, rb));
                    }
                }
                _ => {}
            }
        }
    }

    println!("Diff (threshold {:.2}px):", threshold);
    for (id, dx, dy, dw, dh, lb, rb) in &diffs {
        println!(
            "{} dx={:.2} dy={:.2} dw={:.2} dh={:.2}  left=({:.1},{:.1},{:.1},{:.1}) right=({:.1},{:.1},{:.1},{:.1})",
            id, dx, dy, dw, dh, lb.x, lb.y, lb.w, lb.h, rb.x, rb.y, rb.w, rb.h
        );
    }
    if diffs.is_empty() {
        println!("No differences above threshold.");
    }
    Ok(())
}

/// Capture layout snapshot of a page (reference: JS boxes; renderer: report), fail-fast.
pub async fn capture_layout(opts: CaptureOptions<'_>) -> Result<LayoutCapture> {
    let user_data_dir = format!(
        "/tmp/raybox-cdp-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let mut args = vec![
        "--enable-unsafe-webgpu".to_string(),
        "--enable-webgpu-developer-features".to_string(),
        "--enable-features=Vulkan,VulkanFromANGLE".to_string(),
        "--enable-vulkan".to_string(),
        "--use-angle=vulkan".to_string(),
        "--disable-software-rasterizer".to_string(),
        "--ozone-platform=x11".to_string(),
        "--headless=new".to_string(),
        "--remote-debugging-port=0".to_string(),
        format!("--user-data-dir={}", user_data_dir),
    ];
    if opts.headed {
        args.retain(|a| a != "--headless=new");
    }

    let mut config_builder = BrowserConfig::builder().args(args);
    if let Some(path) = opts.chrome_path {
        config_builder = config_builder.chrome_executable(path);
    }

    let (browser, mut handler) = Browser::launch(
        config_builder
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build browser config: {e}"))?,
    )
    .await
    .context("Failed to launch Chrome")?;

    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = browser
        .new_page("about:blank")
        .await
        .context("Failed to open new page")?;

    // Force viewport / DPR
    let metrics = SetDeviceMetricsOverrideParams::builder()
        .width(opts.viewport.w as i64)
        .height(opts.viewport.h as i64)
        .device_scale_factor(opts.viewport.dpr as f64)
        .mobile(false)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build device metrics: {e}"))?;
    page.execute(metrics).await?;

    // Navigate (add cache-busting query param to avoid stale JS/layout)
    let bust = format!("{}{}v={}", if opts.target_url.contains('?') { "&" } else { "?" }, "", chrono::Utc::now().timestamp_millis());
    let url = format!("{}{}", opts.target_url, bust);
    let nav = NavigateParams::builder()
        .url(&url)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build navigate params: {e}"))?;
    page.execute(nav).await?;
    page.wait_for_navigation().await?;

    // Wait for load event to fire
    let mut load_events = page.event_listener::<EventLoadEventFired>().await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        load_events.next().await;
    })
    .await
    .ok();

    // Extra settle
    tokio::time::sleep(Duration::from_millis(500)).await;
    wait_for_render(&page).await?;

    // Browser version (used in metadata)
    let version = browser
        .version()
        .await
        .map(|v| v.product)
        .unwrap_or_else(|_| "unknown".to_string());

    // Reference capture via JS bounding boxes
    if opts.is_reference {
        let js_capture = r###"
        (() => {
          const styles = ["font-size","font-family","font-weight","line-height","color","background-color","opacity","visibility"];
          const nodes = [];
          let counter = 0;

          const rectObj = (r) => ({ x: r.x, y: r.y, w: r.width, h: r.height });

          function captureElement(el) {
            const rect = el.getBoundingClientRect();
            const has_box = rect.width !== 0 || rect.height !== 0;
            let id = null;
            if (has_box) {
              id = `elem-${counter++}`;
              const cs = getComputedStyle(el);
              const st = {};
              styles.forEach((k) => {
                st[k] = cs.getPropertyValue(k);
              });
              // font metrics via canvas
              const canvas = document.createElement('canvas');
              const ctx = canvas.getContext('2d');
              const font = `${cs.getPropertyValue('font-weight')} ${cs.getPropertyValue('font-size')} ${cs.getPropertyValue('font-family')}`;
              ctx.font = font;
              const fmText = el.textContent && el.textContent.trim() ? el.textContent : 'Hg';
              const m = ctx.measureText(fmText);
              const ascent = m.actualBoundingBoxAscent || (parseFloat(cs.getPropertyValue('font-size')) * 0.8);
              const descent = m.actualBoundingBoxDescent || (parseFloat(cs.getPropertyValue('font-size')) * 0.2);

              nodes.push({
                id,
                node_type: "element",
                tag: el.tagName.toLowerCase(),
                classes: [...el.classList],
                box: rectObj(rect),
                client_rects: [],
                inline_text_boxes: [],
                styles: st,
                font_metrics: { ascent, descent }
              });
            }

            let textIdx = 0;
            for (const child of el.childNodes) {
              if (
                has_box &&
                child.nodeType === Node.TEXT_NODE &&
                child.textContent.trim()
              ) {
                const range = document.createRange();
                range.selectNodeContents(child);
                const trect = range.getBoundingClientRect();
                if (trect.width === 0 && trect.height === 0) continue;
                const client = [...range.getClientRects()].map(rectObj);
                nodes.push({
                  id: `${id}-text-${textIdx++}`,
                  node_type: "text",
                  tag: "#text",
                  classes: [],
                  box: rectObj(trect),
                  client_rects: client,
                  inline_text_boxes: [],
                  styles: {},
                });
              } else if (child.nodeType === Node.ELEMENT_NODE) {
                if (child.tagName && child.tagName.toLowerCase() === 'head') {
                  continue;
                }
                captureElement(child);
              }
            }
          }

          captureElement(document.documentElement);
          return JSON.stringify(nodes);
        })();
        "###;

        let eval = EvaluateParams::builder()
            .expression(js_capture)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build eval params: {e}"))?;

        let res = page
            .execute(eval)
            .await
            .context("reference JS capture failed")?;
        let s = res
            .result
            .result
            .value
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("reference capture missing"))?;

        let nodes_val = serde_json::from_str::<serde_json::Value>(&s)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .ok_or_else(|| anyhow::anyhow!("reference capture parse failed"))?;
        if nodes_val.is_empty() {
            anyhow::bail!("reference capture empty");
        }
        let mut out_nodes = Vec::new();
        for (i, n) in nodes_val.iter().enumerate() {
            if let Some(id) = n.get("id").and_then(|v| v.as_str()) {
                let node_type = n
                    .get("node_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("other")
                    .to_string();
                let tag = n.get("tag").and_then(|v| v.as_str()).map(|s| s.to_string());
                let classes = n
                    .get("classes")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|c| c.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let box_model = n.get("box").and_then(|b| {
                    Some(Rect {
                        x: b.get("x")?.as_f64()?,
                        y: b.get("y")?.as_f64()?,
                        w: b.get("w")?.as_f64()?,
                        h: b.get("h")?.as_f64()?,
                    })
                });
                let client_rects = n
                    .get("client_rects")
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|b| {
                                Some(Rect {
                                    x: b.get("x")?.as_f64()?,
                                    y: b.get("y")?.as_f64()?,
                                    w: b.get("w")?.as_f64()?,
                                    h: b.get("h")?.as_f64()?,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let styles = n
                    .get("styles")
                    .and_then(|m| m.as_object())
                    .cloned()
                    .unwrap_or_default();
                out_nodes.push(Node {
                    id: id.to_string(),
                    source_index: Some(i),
                    backend_node_id: None,
                    node_type,
                    tag,
                    classes,
                    pseudo: None,
                    text: None,
                    box_model,
                    client_rects,
                    inline_text_boxes: Vec::new(),
                    styles,
                    font_metrics: n
                        .get("font_metrics")
                        .and_then(|m| {
                            Some(crate::layout_precise::FontMetrics {
                                ascent: m.get("ascent")?.as_f64()? as f32,
                                descent: m.get("descent")?.as_f64()? as f32,
                            })
                        }),
                });
            }
        }
        let capture = LayoutCapture {
            metadata: Metadata {
                url: opts.target_url.to_string(),
                viewport: opts.viewport.clone(),
                captured_at: chrono::Utc::now().to_rfc3339(),
                chrome: version,
            },
            nodes: out_nodes,
        };
        std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture)?)?;
        return Ok(capture);
    }

    // Renderer capture via raybox_report_json
    // Wait (up to 5s) for the page to expose __layout_json, otherwise log debug info
    let fetch_probe = EvaluateParams::builder()
        .expression("fetch('/reference/todomvc_dom_layout_700.json?v=20251126').then(r=>r.json()).then(j=>j.elements[0].height).catch(e=>`err:${e}`)")
        .await_promise(true)
        .return_by_value(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build fetch probe eval: {e}"))?;
    if let Ok(res) = page.execute(fetch_probe).await {
        println!("[capture] probe html height fetched {:?}", res.result.result.value);
    }

    let layout_passthrough = wait_for_global_string(&page, "__sanitized_layout", Duration::from_millis(5000)).await?;
    if layout_passthrough.is_none() {
        anyhow::bail!("__sanitized_layout not available (page init failed)");
    }
    if let Some(lp) = layout_passthrough.as_deref() {
        println!("[capture] __layout_json_passthrough prefix: {}", &lp[..lp.len().min(80)]);
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(lp) {
            let h = val.get("elements").and_then(|e| e.get(0)).and_then(|e| e.get("height")).and_then(|v| v.as_f64());
            let bh = val.get("elements").and_then(|e| e.get(1)).and_then(|e| e.get("height")).and_then(|v| v.as_f64());
            println!("[capture] passthrough html height {:?}, body height {:?}", h, bh);
        }
        // Dump for inspection
        let _ = std::fs::write("/tmp/raybox_layout_passthrough.json", lp);
    }
    let first_tag = wait_for_global_string(&page, "__first_elem_tag", Duration::from_millis(500)).await?;
    let elem_count = wait_for_global_number(&page, "__elem_count", Duration::from_millis(500)).await?;
    println!("[capture] parsed first tag {:?}, elem_count {:?}", first_tag, elem_count);
    if let Some(s) = wait_for_global_string(&page, "__layout_json", Duration::from_millis(5000)).await? {
        println!("[capture] layout_json length {}", s.len());
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(h) = val
                .get("elements")
                .and_then(|e| e.get(0))
                .and_then(|e| e.get("height"))
                .and_then(|h| h.as_f64())
            {
                println!("[capture] layout first element height {}", h);
            }
        }
    } else {
        let err = wait_for_global_string(&page, "__layout_error", Duration::from_millis(500)).await?;
        let parsed_html = wait_for_global_number(&page, "__parsed_html_height", Duration::from_millis(500)).await?;
        let parsed_body = wait_for_global_number(&page, "__parsed_body_height", Duration::from_millis(500)).await?;
        println!(
            "[capture] __layout_json not found; __layout_error={:?}; __parsed_html_height={:?}; __parsed_body_height={:?}",
            err, parsed_html, parsed_body
        );
    }

    let report_script = r#"
    (function () {
      if (typeof raybox_report_json === 'function') return raybox_report_json();
      if (typeof wasm_bindgen !== 'undefined' && typeof wasm_bindgen.raybox_report_json === 'function') return wasm_bindgen.raybox_report_json();
      return null;
    })();
    "#;
    let eval = EvaluateParams::builder()
        .expression(report_script)
        .await_promise(true)
        .return_by_value(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build eval params: {e}"))?;
    let res = page
        .execute(eval)
        .await
        .context("renderer report eval failed")?;
    let s = res
        .result
        .result
        .value
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("renderer report missing"))?;
    let nodes =
        parse_report_nodes(&s).ok_or_else(|| anyhow::anyhow!("renderer report parse failed"))?;
    if nodes.is_empty() {
        anyhow::bail!("renderer report empty");
    }
    let capture = LayoutCapture {
        metadata: Metadata {
            url: opts.target_url.to_string(),
            viewport: opts.viewport.clone(),
            captured_at: chrono::Utc::now().to_rfc3339(),
            chrome: version,
        },
        nodes,
    };
    std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture)?)?;
    Ok(capture)
}

/// Poll for a global string variable (e.g., __layout_json) with timeout.
async fn wait_for_global_string(page: &Page, name: &str, timeout: Duration) -> Result<Option<String>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let expr = format!(
            "(function(){{return typeof {} === 'string' ? {} : null;}})();",
            name, name
        );
        let eval = EvaluateParams::builder()
            .expression(expr)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build eval params: {e}"))?;
        if let Ok(res) = page.execute(eval).await {
            if let Some(s) = res
                .result
                .result
                .value
                .and_then(|v| v.as_str().map(|s| s.to_string()))
            {
                return Ok(Some(s));
            }
        }
        if std::time::Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Poll for a global number variable with timeout.
async fn wait_for_global_number(page: &Page, name: &str, timeout: Duration) -> Result<Option<f64>> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let expr = format!(
            "(function(){{return (typeof {0} === 'number') ? {0} : null;}})();",
            name
        );
        let eval = EvaluateParams::builder()
            .expression(expr)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build eval params: {e}"))?;
        if let Ok(res) = page.execute(eval).await {
            if let Some(n) = res
                .result
                .result
                .value
                .and_then(|v| v.as_f64())
            {
                return Ok(Some(n));
            }
        }
        if std::time::Instant::now() >= deadline {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn parse_report_nodes(report_json: &str) -> Option<Vec<Node>> {
    let val = serde_json::from_str::<serde_json::Value>(report_json).ok()?;
    let nodes = val.get("nodes")?.as_array()?;
    let mut extra = Vec::new();
    for node in nodes {
        if let (Some(id), Some(x), Some(y), Some(w), Some(h)) = (
            node.get("id").and_then(|v| v.as_str()),
            node.get("x").and_then(|v| v.as_f64()),
            node.get("y").and_then(|v| v.as_f64()),
            node.get("w").and_then(|v| v.as_f64()),
            node.get("h").and_then(|v| v.as_f64()),
        ) {
            extra.push(Node {
                id: id.to_string(),
                source_index: node
                    .get("source_index")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize),
                backend_node_id: None,
                node_type: "virtual".to_string(),
                tag: node
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                classes: node
                    .get("classes")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(|t| t.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                pseudo: None,
                text: None,
                box_model: Some(Rect { x, y, w, h }),
                client_rects: vec![],
                inline_text_boxes: vec![],
                styles: serde_json::Map::new(),
                font_metrics: None,
            });
        }
    }
    Some(extra)
}

async fn wait_for_render(page: &Page) -> Result<()> {
    let script = r#"
        new Promise((resolve) => {
          requestAnimationFrame(() => requestAnimationFrame(resolve));
        });
    "#;
    let _ = page.evaluate(script).await?;
    Ok(())
}

fn find_free_port(start: u16) -> Result<u16> {
    for port in start..start + 1000 {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    anyhow::bail!("No free port found");
}

/// Start a temporary static server for a directory and return (addr, shutdown_sender).
pub async fn start_static_server(
    dir: &Path,
) -> Result<(SocketAddr, tokio::sync::oneshot::Sender<()>)> {
    let serve_path = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(dir)
    };
    if !serve_path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", serve_path.display());
    }

    let port = find_free_port(8000)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let serve_dir = ServeDir::new(&serve_path).append_index_html_on_directories(true);
    let app = Router::new().nest_service("/", get_service(serve_dir));
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let server = axum::serve(listener, app);
        tokio::select! {
            _ = server => {},
            _ = rx => {},
        }
    });

    Ok((addr, tx))
}
