use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{routing::get_service, Router};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::dom::{BackendNodeId, GetBoxModelParams};
use chromiumoxide::cdp::browser_protocol::page::EventLoadEventFired;
use chromiumoxide::cdp::browser_protocol::{
    dom_snapshot::CaptureSnapshotParams, emulation::SetDeviceMetricsOverrideParams,
    page::NavigateParams,
};
use chromiumoxide::cdp::js_protocol::runtime::{EvaluateParams, RemoteObject};
use chromiumoxide::Page;
use futures::StreamExt;
use serde_json::json;
use tower_http::services::ServeDir;

use crate::commands::capture_js::JS_COLLECT_RECTS;
use crate::layout_precise::{LayoutCapture, Metadata, Node, Rect, Viewport};

/// Options for a single capture run.
pub struct CaptureOptions<'a> {
    pub target_url: &'a str,
    pub out_path: &'a Path,
    pub viewport: Viewport,
    pub headed: bool,
    pub chrome_path: Option<&'a str>,
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
            });
        }
    }
    Some(extra)
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

    // Pick a free port starting at 8000.
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

/// Capture layout snapshot of a page.
pub async fn capture_layout(opts: CaptureOptions<'_>) -> Result<LayoutCapture> {
    let mut args = vec![
        "--enable-unsafe-webgpu",
        "--enable-webgpu-developer-features",
        "--enable-features=Vulkan,VulkanFromANGLE",
        "--enable-vulkan",
        "--use-angle=vulkan",
        "--disable-software-rasterizer",
        "--ozone-platform=x11",
        "--headless=new",
        "--remote-debugging-port=0",
        "--user-data-dir=/tmp/raybox-cdp",
    ];

    if opts.headed {
        args.retain(|a| *a != "--headless=new");
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

    // Navigate
    let nav = NavigateParams::builder()
        .url(opts.target_url)
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

    // Try to pull renderer report JSON directly (preferred path)
    let report_script = r#"
    (async () => {
      const get = () => {
        if (typeof raybox_report_json === 'function') return raybox_report_json();
        if (typeof wasm_bindgen !== 'undefined' && typeof wasm_bindgen.raybox_report_json === 'function') return wasm_bindgen.raybox_report_json();
        const h = document.documentElement;
        if (h) {
          const v = h.getAttribute('data-raybox-report');
          if (v) return v;
        }
        return null;
      };
      let v = get();
      if (v && v !== "{}") return v;
      for (let i = 0; i < 50; i++) {
        await new Promise(r => setTimeout(r, 100));
        v = get();
        if (v && v !== "{}") return v;
      }
      return get();
    })();
    "#;

    let mut direct_report_nodes: Option<Vec<Node>> = None;
    let eval = EvaluateParams::builder()
        .expression(report_script)
        .await_promise(true)
        .return_by_value(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build eval params: {e}"))?;
    if let Ok(res) = page.execute(eval).await {
        let remote: RemoteObject = res.result.result;
        if let Some(val) = remote.value {
            if let Some(s) = val.as_str() {
                if let Some(extra) = parse_report_nodes(s) {
                    if !extra.is_empty() {
                        direct_report_nodes = Some(extra);
                    }
                }
            }
        }
    }

    // Try JS-based precise capture (bounding boxes via DOM APIs) — reference only
    if opts.target_url.contains("reference/") {
        let js_capture = r###"
        (() => {
          const styles = ["font-size","font-family","font-weight","line-height","color","background-color","opacity","visibility"];
          function pathFor(node){
            if(!node || node===document) return "root";
            const parent=node.parentNode;
            const parentPath=pathFor(parent);
            const tag = node.nodeType===Node.TEXT_NODE ? "#text" : (node.tagName||"node").toLowerCase();
            const siblings=[...parent.childNodes].filter(n=>n.nodeType===node.nodeType && (n.tagName===node.tagName || n.nodeType===Node.TEXT_NODE));
            const idx=siblings.indexOf(node);
            return `${parentPath}/${tag}[${idx>=0?idx:0}]`;
          }
          function rectObj(r){return {x:r.x,y:r.y,w:r.width,h:r.height};}
          function elemData(el){
            const rect = el.getBoundingClientRect();
            const cs = getComputedStyle(el);
            const st={};
            styles.forEach(k=>{st[k]=cs.getPropertyValue(k);});
            return {id:pathFor(el), node_type:"element", tag:el.tagName.toLowerCase(), classes:[...el.classList], box:rectObj(rect), client_rects:[], inline_text_boxes:[], styles:st};
          }
          function textData(tn){
            const range=document.createRange();
            range.selectNodeContents(tn);
            const rect=range.getBoundingClientRect();
            const client=[...range.getClientRects()].map(rectObj);
            return {id:pathFor(tn), node_type:"text", tag:"#text", classes:[], box:rectObj(rect), client_rects:client, inline_text_boxes:[], styles:{}};
          }
          const nodes=[];
          const walker=document.createTreeWalker(document, NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT);
          let n; while((n=walker.nextNode())){
            if(n.nodeType===1) nodes.push(elemData(n));
            else if(n.nodeType===3 && n.textContent.trim()) nodes.push(textData(n));
          }
          return JSON.stringify(nodes);
        })();
        "###;
        if let Ok(res) = page.evaluate(js_capture).await {
            if let Some(val) = res.value() {
                if let Some(s) = val.as_str() {
                    if let Ok(list) = serde_json::from_str::<serde_json::Value>(s) {
                        if let Some(arr) = list.as_array() {
                            let mut nodes = Vec::new();
                            for (i, n) in arr.iter().enumerate() {
                                if let Some(id) = n.get("id").and_then(|v| v.as_str()) {
                                    let node_type = n.get("node_type").and_then(|v| v.as_str()).unwrap_or("other").to_string();
                                    let tag = n.get("tag").and_then(|v| v.as_str()).map(|s| s.to_string());
                                    let classes = n.get("classes").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|c| c.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
                                    let box_model = n.get("box").and_then(|b| {
                                        Some(Rect{
                                            x:b.get("x")?.as_f64()?,
                                            y:b.get("y")?.as_f64()?,
                                            w:b.get("w")?.as_f64()?,
                                            h:b.get("h")?.as_f64()?,
                                        })
                                    });
                                    let client_rects = n.get("client_rects").and_then(|a| a.as_array()).map(|a| {
                                        a.iter().filter_map(|b| {
                                            Some(Rect{
                                                x:b.get("x")?.as_f64()?,
                                                y:b.get("y")?.as_f64()?,
                                                w:b.get("w")?.as_f64()?,
                                                h:b.get("h")?.as_f64()?,
                                            })
                                        }).collect()
                                    }).unwrap_or_default();
                                    let styles = n.get("styles").and_then(|m| m.as_object()).cloned().unwrap_or_default();
                                    nodes.push(Node{
                                        id:id.to_string(),
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
                                    });
                                }
                            }
                            if !nodes.is_empty() {
                                let capture = LayoutCapture{
                                    metadata: Metadata{
                                        url: opts.target_url.to_string(),
                                        viewport: opts.viewport.clone(),
                                        captured_at: chrono::Utc::now().to_rfc3339(),
                                        chrome: "js-capture".into(),
                                    },
                                    nodes,
                                };
                                std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture)?)?;
                                return Ok(capture);
                            }
                        }
                    }
                }
            }
        }
    }

    // If we already have a direct renderer report, build capture and return
    if let Some(nodes) = direct_report_nodes {
        let capture = LayoutCapture {
            metadata: Metadata {
                url: opts.target_url.to_string(),
                viewport: opts.viewport.clone(),
                captured_at: chrono::Utc::now().to_rfc3339(),
                chrome: version.clone(),
            },
            nodes,
        };
        std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture)?)?;
        return Ok(capture);
    }

    // Capture DOM snapshot (fallback path)
    let computed_styles = vec![
        "font-size",
        "font-family",
        "font-weight",
        "line-height",
        "color",
        "background-color",
        "border-top-width",
        "border-right-width",
        "border-bottom-width",
        "border-left-width",
        "border-top-color",
        "border-right-color",
        "border-bottom-color",
        "border-left-color",
        "border-radius",
        "box-shadow",
        "opacity",
        "visibility",
        "z-index",
    ];

    let snapshot_params = CaptureSnapshotParams::builder()
        .computed_styles(computed_styles.clone())
        .include_dom_rects(true)
        .include_paint_order(true)
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build capture params: {e}"))?;

    let snapshot = page.execute(snapshot_params).await?.result;

    // Fallback box models for nodes missing layout boxes
    let fallback_boxes = fetch_missing_boxes(&page, &snapshot).await?;

    // JS fallback by path
    let fallback_paths = fetch_js_fallback_rects(&page).await?;

    let mut capture = decode_snapshot(
        snapshot,
        &computed_styles,
        &opts.viewport,
        opts.target_url,
        &version,
        &fallback_boxes,
        &fallback_paths,
    )?;

    // Collect possible report sources
    let mut report_sources: Vec<String> = Vec::new();
    if let Ok(val) = page
        .evaluate("document.documentElement && document.documentElement.getAttribute('data-raybox-report')")
        .await
    {
        if let Some(s) = val.value().and_then(|v| v.as_str()).map(|s| s.to_string()) {
            report_sources.push(s);
        }
    }
    if let Ok(val) = page
        .evaluate("globalThis.__raybox_report_json || window.__raybox_report_json || null")
        .await
    {
        if let Some(s) = val.value().and_then(|v| v.as_str()).map(|s| s.to_string()) {
            report_sources.push(s);
        }
    }

    // Merge any report JSON found
    for s in report_sources {
        if let Some(extra) = parse_report_nodes(&s) {
            if !extra.is_empty() {
                capture.nodes.extend(extra);
                break;
            }
        }
    }

    // Try to merge embedded JSON report from script text nodes
    for n in capture.nodes.clone() {
        if n.node_type == "text" {
            if let Some(txt) = &n.text {
                if txt.contains("RAYBOX_REPORT:") {
                    if let Some(pos) = txt.find("RAYBOX_REPORT:") {
                        let json_part = txt[pos + "RAYBOX_REPORT:".len()..].trim();
                        if let Some(extra) = parse_report_nodes(json_part) {
                            if !extra.is_empty() {
                                capture.nodes.extend(extra);
                                break;
                            }
                        }
                    }
                } else if txt.len() > 50 && txt.trim_start().starts_with('{') {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(txt) {
                        if let Some(nodes) = val.get("nodes").and_then(|v| v.as_array()) {
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
                                        node_type: "virtual".into(),
                                        tag: node
                                            .get("tag")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string()),
                                        classes: node
                                            .get("classes")
                                            .and_then(|v| v.as_array())
                                            .map(|a| {
                                                a.iter()
                                                    .filter_map(|s| {
                                                        s.as_str().map(|t| t.to_string())
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                        pseudo: None,
                                        text: None,
                                        box_model: Some(Rect { x, y, w, h }),
                                        client_rects: vec![],
                                        inline_text_boxes: vec![],
                                        styles: serde_json::Map::new(),
                                    });
                                }
                            }
                            if !extra.is_empty() {
                                capture.nodes.extend(extra);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Try direct wasm_bindgen report JSON first
    if let Ok(render_report) = page
        .evaluate(
            "(function(){if (typeof wasm_bindgen !== 'undefined' && wasm_bindgen.raybox_report_json) { return wasm_bindgen.raybox_report_json(); } return null;})()",
        )
        .await
    {
        if let Some(s) = render_report.value().and_then(|v| v.as_str()).map(|s| s.to_string()) {
            if let Some(extra) = parse_report_nodes(&s) {
                if !extra.is_empty() {
                    capture.nodes.extend(extra);
                    std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture)?)?;
                    return Ok(capture);
                }
            }
        }
    }

    // If the page exposes get_render_report(), merge its nodes with IDs (e.g., elem-<idx>)
    let report_script = r#"
    (async () => {
      const candidates = [
        () => window.__raybox_report_data ? window.__raybox_report_data : null,
        () => window.__raybox_get_render_report ? window.__raybox_get_render_report() : null,
        () => (typeof wasm_bindgen !== 'undefined' && typeof wasm_bindgen.get_render_report === 'function') ? wasm_bindgen.get_render_report() : null,
        () => (typeof get_render_report === 'function') ? get_render_report() : null,
        async () => {
          try {
            const mod = await import('/pkg/renderer.js');
            if (mod.get_render_report) return mod.get_render_report();
          } catch (e) {}
          return null;
        }
      ];
      for (const fn of candidates) {
        try {
          const v = await fn();
          if (v) return v;
        } catch (e) {}
      }
      return null;
    })();
    "#;

    if let Ok(render_report) = page.evaluate(report_script).await {
        if let Some(val) = render_report.value() {
            if let Some(obj) = val.as_object() {
                if let Some(nodes_val) = obj.get("nodes") {
                    if let Some(arr) = nodes_val.as_array() {
                        let mut extra = Vec::new();
                        for n in arr {
                            if let (Some(id), Some(x), Some(y), Some(w), Some(h)) = (
                                n.get("id").and_then(|v| v.as_str()),
                                n.get("x").and_then(|v| v.as_f64()),
                                n.get("y").and_then(|v| v.as_f64()),
                                n.get("w").and_then(|v| v.as_f64()),
                                n.get("h").and_then(|v| v.as_f64()),
                            ) {
                                extra.push(Node {
                                    id: id.to_string(),
                                    source_index: n
                                        .get("source_index")
                                        .and_then(|v| v.as_u64())
                                        .map(|v| v as usize),
                                    backend_node_id: None,
                                    node_type: "virtual".to_string(),
                                    tag: n
                                        .get("tag")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    classes: n
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
                                });
                            }
                        }
                        let mut capture_mut = capture;
                        capture_mut.nodes.extend(extra);
                        // write combined capture
                        std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture_mut)?)?;
                        return Ok(capture_mut);
                    }
                }
            }
        }
    }

    // Fallback: direct module import and call
    if let Ok(render_report) = page
        .evaluate(
            "(async()=>{try{const m=await import('/pkg/renderer.js'); if(m.default) await m.default(); if(m.get_render_report) return m.get_render_report();}catch(e){} return null;})()",
        )
        .await
    {
        if let Some(val) = render_report.value() {
            if let Some(obj) = val.as_object() {
                if let Some(nodes_val) = obj.get("nodes") {
                    if let Some(arr) = nodes_val.as_array() {
                        let mut extra = Vec::new();
                        for n in arr {
                            if let (Some(id), Some(x), Some(y), Some(w), Some(h)) = (
                                n.get("id").and_then(|v| v.as_str()),
                                n.get("x").and_then(|v| v.as_f64()),
                                n.get("y").and_then(|v| v.as_f64()),
                                n.get("w").and_then(|v| v.as_f64()),
                                n.get("h").and_then(|v| v.as_f64()),
                            ) {
                                extra.push(Node {
                                    id: id.to_string(),
                                    source_index: n.get("source_index").and_then(|v| v.as_u64()).map(|v| v as usize),
                                    backend_node_id: None,
                                    node_type: "virtual".to_string(),
                                    tag: n.get("tag").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    classes: n
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
                                });
                            }
                        }
                        let mut capture_mut = capture;
                        capture_mut.nodes.extend(extra);
                        std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture_mut)?)?;
                        return Ok(capture_mut);
                    }
                }
            }
        }
    }

    // Write out
    std::fs::write(opts.out_path, serde_json::to_string_pretty(&capture)?)?;

    Ok(capture)
}

/// Decode CDP snapshot into our schema.
fn decode_snapshot(
    snap: chromiumoxide::cdp::browser_protocol::dom_snapshot::CaptureSnapshotReturns,
    computed_styles: &[&str],
    viewport: &Viewport,
    url: &str,
    chrome: &str,
    fallback_boxes: &HashMap<u64, Rect>,
    fallback_paths: &HashMap<String, Rect>,
) -> Result<LayoutCapture> {
    let strings = snap.strings;
    let doc = snap.documents.get(0).context("No document in snapshot")?;
    let nodes = &doc.nodes;
    let layout = &doc.layout;
    let text_boxes = &doc.text_boxes;

    let string_at = |idx: i64| -> String { strings.get(idx as usize).cloned().unwrap_or_default() };

    let node_count = nodes.node_name.as_ref().map(|v| v.len()).unwrap_or(0);

    // Precompute rare string/int lookups
    let rare_string_lookup =
        |data: &Option<chromiumoxide::cdp::browser_protocol::dom_snapshot::RareStringData>,
         idx: usize|
         -> Option<String> {
            let data = data.as_ref()?;
            data.index
                .iter()
                .position(|i| *i as usize == idx)
                .and_then(|pos| data.value.get(pos))
                .map(|s| string_at(*s.inner()))
        };

    // Map layout node index -> layout entry
    let mut layout_by_node: HashMap<usize, usize> = HashMap::new();
    for (i, node_idx) in layout.node_index.iter().enumerate() {
        layout_by_node.insert(*node_idx as usize, i);
    }

    // Inline text boxes grouped by layout index
    let mut inline_boxes: HashMap<usize, Vec<Rect>> = HashMap::new();
    for (i, layout_idx) in text_boxes.layout_index.iter().enumerate() {
        let bounds = text_boxes
            .bounds
            .get(i)
            .map(rect_from_proto)
            .unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            });
        inline_boxes
            .entry(*layout_idx as usize)
            .or_default()
            .push(bounds);
    }

    // Build children list for path computation
    let parent_index = nodes.parent_index.clone().unwrap_or_default();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    for (child_idx_i64, parent_i64) in parent_index.iter().enumerate() {
        if *parent_i64 >= 0 {
            let p = *parent_i64 as usize;
            children[p].push(child_idx_i64);
        }
    }

    let mut tag_cache: Vec<Option<String>> = vec![None; node_count];
    for idx in 0..node_count {
        let name = nodes
            .node_name
            .as_ref()
            .and_then(|v| v.get(idx))
            .map(|s| string_at(*s.inner()).to_lowercase());
        tag_cache[idx] = name;
    }

    let mut node_path_cache: Vec<Option<String>> = vec![None; node_count];
    node_path_cache[0] = Some("root".into());

    let mut out_nodes = Vec::with_capacity(node_count);
    for idx in 0..node_count {
        let node_type_val = nodes
            .node_type
            .as_ref()
            .and_then(|v| v.get(idx))
            .cloned()
            .unwrap_or(0);
        let node_type = match node_type_val {
            1 => "element",
            3 => "text",
            _ => "other",
        }
        .to_string();

        let name = tag_cache.get(idx).cloned().unwrap_or(None);

        let backend_id = nodes
            .backend_node_id
            .as_ref()
            .and_then(|v| v.get(idx))
            .map(|b| *b.inner() as u64);

        // pseudo type if any
        let pseudo = rare_string_lookup(&nodes.pseudo_type, idx);

        // attributes
        let mut classes = Vec::new();
        let mut data_report_attr: Option<String> = None;
        if let Some(attrs_vec) = nodes.attributes.as_ref() {
            if let Some(arr) = attrs_vec.get(idx) {
                let mut it = arr.inner().iter();
                while let Some(name_idx) = it.next() {
                    if let Some(val_idx) = it.next() {
                        let name_str = string_at(*name_idx.inner());
                        let val_str = string_at(*val_idx.inner());
                        if name_str == "class" {
                            classes = val_str.split_whitespace().map(|s| s.to_string()).collect();
                        } else if name_str == "data-raybox-report" {
                            data_report_attr = Some(val_str);
                        }
                    }
                }
            }
        }

        let text = if node_type == "text" {
            nodes
                .node_value
                .as_ref()
                .and_then(|v| v.get(idx))
                .map(|s| string_at(*s.inner()))
        } else {
            None
        };

        // Layout box
        let (box_model, client_rects, inline_text_boxes) =
            if let Some(layout_idx) = layout_by_node.get(&idx) {
                let rect = layout.bounds.get(*layout_idx).map(rect_from_proto);
                let client_rects = layout
                    .client_rects
                    .as_ref()
                    .and_then(|list| list.get(*layout_idx))
                    .map(|r| vec![rect_from_proto(r)])
                    .unwrap_or_default();
                let inline = inline_boxes.get(layout_idx).cloned().unwrap_or_default();
                (rect, client_rects, inline)
            } else {
                (None, Vec::new(), Vec::new())
            };

        let id = compute_path(
            idx,
            &parent_index,
            &children,
            &mut node_path_cache,
            &tag_cache,
        );

        // Filter: we only care about body subtree (and root/html for context) and skip pseudo nodes for now.
        let in_body = id.contains("/body[");
        if pseudo.is_some() || !(in_body || id == "root" || tag_cache.get(idx).and_then(|t| t.as_ref()).map(|s| s.as_str()) == Some("html")) {
            continue;
        }

        let box_model = box_model
            .or_else(|| backend_id.and_then(|id| fallback_boxes.get(&id).cloned()))
            .or_else(|| fallback_paths.get(&id).cloned());

        // Styles
        let styles_map = if let Some(layout_idx) = layout_by_node.get(&idx) {
            layout
                .styles
                .get(*layout_idx)
                .map(|arr| {
                    let mut map = serde_json::Map::new();
                    for (i, s_idx) in arr.inner().iter().enumerate() {
                        if let Some(name) = computed_styles.get(i) {
                            map.insert(name.to_string(), json!(string_at(*s_idx.inner())));
                        }
                    }
                    map
                })
                .unwrap_or_default()
        } else {
            serde_json::Map::new()
        };

        out_nodes.push(Node {
            id,
            source_index: Some(idx),
            backend_node_id: backend_id,
            node_type: node_type.clone(),
            tag: name.clone(),
            classes,
            pseudo,
            text,
            box_model: box_model.clone(),
            client_rects: client_rects.clone(),
            inline_text_boxes: inline_text_boxes.clone(),
            styles: styles_map,
        });

        // If this is a text node, synthesize elem-<parent>-text/-link based on parent index and its tag.
        if node_type == "text" {
            if let Some(parent_i64) = parent_index.get(idx) {
                if *parent_i64 >= 0 {
                    let p_idx = *parent_i64 as usize;
                    if let Some(tb) = box_model
                        .clone()
                        .or_else(|| inline_text_boxes.get(0).cloned())
                        .or_else(|| client_rects.get(0).cloned())
                    {
                        let parent_tag = tag_cache.get(p_idx).cloned().unwrap_or(None);
                        let parent_id = format!("elem-{}-text", p_idx);
                        out_nodes.push(Node {
                            id: parent_id.clone(),
                            source_index: Some(p_idx),
                            backend_node_id: None,
                            node_type: "text".into(),
                            tag: parent_tag.clone(),
                            classes: Vec::new(),
                            pseudo: None,
                            text: None,
                            box_model: Some(tb),
                            client_rects: vec![],
                            inline_text_boxes: vec![],
                            styles: serde_json::Map::new(),
                        });
                        if parent_tag.as_deref() == Some("a") {
                            out_nodes.push(Node {
                                id: format!("elem-{}-link", p_idx),
                                source_index: Some(p_idx),
                                backend_node_id: None,
                                node_type: "text".into(),
                                tag: Some("a".into()),
                                classes: Vec::new(),
                                pseudo: None,
                                text: None,
                                box_model: Some(tb),
                                client_rects: vec![],
                                inline_text_boxes: vec![],
                                styles: serde_json::Map::new(),
                            });
                        }
                    }
                }
            }
        }

        // Synthesize text nodes for comparison (align with renderer's elem-*-text/link)
        if let Some(src_idx) = out_nodes.last().and_then(|n| n.id.strip_prefix("elem-")).and_then(|_| out_nodes.last().and_then(|n| n.source_index)) {
            if let Some(first_box) = out_nodes.last().and_then(|n| n.inline_text_boxes.get(0)).cloned().or(out_nodes.last().and_then(|n| n.client_rects.get(0)).cloned()) {
                let base_id = out_nodes.last().unwrap().id.clone();
                // text box
                out_nodes.push(Node {
                    id: format!("{}-text", base_id),
                    source_index: Some(src_idx),
                    backend_node_id: None,
                    node_type: "text".into(),
                    tag: out_nodes.last().unwrap().tag.clone(),
                    classes: out_nodes.last().unwrap().classes.clone(),
                    pseudo: None,
                    text: None,
                    box_model: Some(first_box),
                    client_rects: vec![],
                    inline_text_boxes: vec![],
                    styles: serde_json::Map::new(),
                });
                // link box for anchors (for TodoMVC footer link)
                if out_nodes[out_nodes.len() - 2].tag.as_deref() == Some("a") {
                    out_nodes.push(Node {
                        id: format!("{}-link", base_id),
                        source_index: Some(src_idx),
                        backend_node_id: None,
                        node_type: "text".into(),
                        tag: Some("a".into()),
                        classes: out_nodes[out_nodes.len() - 2].classes.clone(),
                        pseudo: None,
                        text: None,
                        box_model: Some(first_box),
                        client_rects: vec![],
                        inline_text_boxes: vec![],
                        styles: serde_json::Map::new(),
                    });
                }
            }
        }

        if let Some(rep) = data_report_attr {
            if let Some(extra) = parse_report_nodes(&rep) {
                out_nodes.extend(extra);
            }
        }
    }

    let capture = LayoutCapture {
        metadata: Metadata {
            url: url.to_string(),
            viewport: viewport.clone(),
            captured_at: chrono::Utc::now().to_rfc3339(),
            chrome: chrome.to_string(),
        },
        nodes: out_nodes,
    };

    Ok(capture)
}

fn rect_from_proto(r: &chromiumoxide::cdp::browser_protocol::dom_snapshot::Rectangle) -> Rect {
    let vals = r.inner();
    let get = |i| *vals.get(i).unwrap_or(&0.0);
    Rect::from_tuple((get(0), get(1), get(2), get(3)))
}

async fn fetch_missing_boxes(
    page: &Page,
    snap: &chromiumoxide::cdp::browser_protocol::dom_snapshot::CaptureSnapshotReturns,
) -> Result<HashMap<u64, Rect>> {
    let mut missing = Vec::new();
    if let Some(nodes) = snap
        .documents
        .get(0)
        .and_then(|d| d.nodes.backend_node_id.as_ref())
    {
        let layout_nodes = snap
            .documents
            .get(0)
            .map(|d| &d.layout)
            .map(|l| &l.node_index)
            .cloned()
            .unwrap_or_default();
        let layout_set: std::collections::HashSet<i64> = layout_nodes.into_iter().collect();
        for (idx, bid) in nodes.iter().enumerate() {
            if !layout_set.contains(&(idx as i64)) {
                missing.push(*bid.inner() as u64);
            }
        }
    }

    let mut map = HashMap::new();
    for bid in missing {
        if let Ok(resp) = page
            .execute(
                GetBoxModelParams::builder()
                    .backend_node_id(BackendNodeId::new(bid as i64))
                    .build(),
            )
            .await
        {
            let model = resp.result.model;
            let content = model.content.inner();
            if content.len() >= 8 {
                let xs = [content[0], content[2], content[4], content[6]];
                let ys = [content[1], content[3], content[5], content[7]];
                let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                map.insert(
                    bid,
                    Rect::from_tuple((min_x, min_y, max_x - min_x, max_y - min_y)),
                );
            }
        }
    }
    Ok(map)
}

async fn fetch_js_fallback_rects(page: &Page) -> Result<HashMap<String, Rect>> {
    let res = page.evaluate(JS_COLLECT_RECTS).await?;
    let val = res
        .value()
        .ok_or_else(|| anyhow::anyhow!("No value from JS rect collector"))?;
    let mut map = HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj {
            if let (Some(x), Some(y), Some(w), Some(h)) =
                (v.get("x"), v.get("y"), v.get("w"), v.get("h"))
            {
                if let (Some(xf), Some(yf), Some(wf), Some(hf)) =
                    (x.as_f64(), y.as_f64(), w.as_f64(), h.as_f64())
                {
                    map.insert(k.clone(), Rect::from_tuple((xf, yf, wf, hf)));
                }
            }
        }
    }
    Ok(map)
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

fn compute_path(
    idx: usize,
    parent_index: &[i64],
    children: &[Vec<usize>],
    cache: &mut [Option<String>],
    tags: &[Option<String>],
) -> String {
    if let Some(p) = cache[idx].clone() {
        return p;
    }
    let parent = parent_index.get(idx).copied().unwrap_or(-1);
    let parent_path = if parent >= 0 {
        compute_path(parent as usize, parent_index, children, cache, tags)
    } else {
        "root".to_string()
    };

    let sibling_index = if parent >= 0 {
        children
            .get(parent as usize)
            .and_then(|kids| kids.iter().position(|c| *c == idx))
            .unwrap_or(0)
    } else {
        0
    };

    let tag_part = tags
        .get(idx)
        .and_then(|t| t.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("node");
    let path = format!("{}/{}[{}]", parent_path, tag_part, sibling_index);
    cache[idx] = Some(path.clone());
    path
}

fn find_free_port(start: u16) -> Result<u16> {
    for port in start..start + 1000 {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    anyhow::bail!("No free port found");
}

/// Capture reference layout by serving a directory (usually `reference/`).
pub async fn run_capture_reference(
    file: &Path,
    out: &Path,
    headed: bool,
    chrome_path: Option<&str>,
    layout_json: Option<&str>,
) -> Result<LayoutCapture> {
    let dir = file.parent().unwrap_or_else(|| Path::new("."));
    let (addr, shutdown) = start_static_server(dir).await?;
    let url = format!("http://{}:{}", addr.ip(), addr.port());
    let url = format!("{}/{}", url, file.file_name().unwrap().to_string_lossy());

    let mut capture = capture_layout(CaptureOptions {
        target_url: &url,
        out_path: out,
        viewport: Viewport {
            w: 700,
            h: 700,
            dpr: 1.0,
        },
        headed,
        chrome_path,
    })
    .await?;

    // Merge analyzer layout nodes (elem-<idx>) to align source_index with renderer layout indices.
    let layout_path = layout_json
        .map(Path::new)
        .filter(|p| p.exists())
        .map(|p| p.to_path_buf())
        .or_else(|| {
            let default = Path::new("reference/todomvc_dom_layout_700.json");
            if default.exists() {
                Some(default.to_path_buf())
            } else {
                None
            }
        });
    if let Some(p) = layout_path {
        if let Ok(extra) = build_from_layout_json(p.to_str().unwrap()) {
            capture.nodes.extend(extra.nodes);
        }
    }

    std::fs::write(out, serde_json::to_string_pretty(&capture)?)?;
    let _ = shutdown.send(());
    Ok(capture)
}

/// Capture renderer layout (assumes renderer already served, e.g., via wasm-start).
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
    })
    .await
}

/// Diff two captures and print a git-like summary.
pub fn run_diff_layouts(a: &Path, b: &Path, threshold: f64) -> Result<()> {
    let left: LayoutCapture = serde_json::from_reader(std::fs::File::open(a)?)?;
    let right: LayoutCapture = serde_json::from_reader(std::fs::File::open(b)?)?;

    // Decide which side is renderer vs reference for matching.
    let left_elem_count = left.nodes.iter().filter(|n| n.id.starts_with("elem-")).count();
    let right_elem_count = right.nodes.iter().filter(|n| n.id.starts_with("elem-")).count();

    // Prefer the side with more elem-* nodes as renderer (primary).
    let (mut primary_vec, mut secondary_pool) = if right_elem_count > left_elem_count {
        (right.nodes.clone(), left.nodes.clone())
    } else {
        (left.nodes.clone(), right.nodes.clone())
    };

    // Restrict comparison strictly to renderer-model element nodes (elem-* + node_type==element).
    primary_vec.retain(|n| n.id.starts_with("elem-") && n.node_type == "element");
    secondary_pool.retain(|n| n.id.starts_with("elem-") && n.node_type == "element");
    let primary: Vec<&Node> = primary_vec.iter().collect();

    let mut diffs = Vec::new();
    for pn in primary {
        // Find best match in secondary
        let mut match_pos: Option<usize> = None;
        if let Some(idx) = pn.source_index {
            match_pos = secondary_pool
                .iter()
                .position(|r| r.source_index == Some(idx));
        }
        if match_pos.is_none() {
            match_pos = secondary_pool.iter().position(|r| r.id == pn.id);
        }
        if match_pos.is_none() {
            let mut best: Option<(usize, f64)> = None;
            for (i, r) in secondary_pool.iter().enumerate() {
                if pn.tag.is_some() && r.tag.is_some() && pn.tag != r.tag {
                    continue;
                }
                let class_overlap = !pn.classes.is_empty()
                    && !r.classes.is_empty()
                    && pn.classes.iter().any(|c| r.classes.contains(c));
                if !pn.classes.is_empty() && !r.classes.is_empty() && !class_overlap {
                    continue;
                }
                let dist = match (pn.box_model, r.box_model) {
                    (Some(lb), Some(rb)) => {
                        (lb.x - rb.x).abs()
                            + (lb.y - rb.y).abs()
                            + (lb.w - rb.w).abs()
                            + (lb.h - rb.h).abs()
                    }
                    _ => f64::INFINITY,
                };
                if best.map(|(_, d)| dist < d).unwrap_or(true) {
                    best = Some((i, dist));
                }
            }
            match_pos = best.map(|(i, _)| i);
        }

        if let Some(pos) = match_pos {
            let rn = secondary_pool.remove(pos);
            if let (Some(lb), Some(rb)) = (pn.box_model, rn.box_model) {
                let dx = (lb.x - rb.x).abs();
                let dy = (lb.y - rb.y).abs();
                let dw = (lb.w - rb.w).abs();
                let dh = (lb.h - rb.h).abs();
                if dx > threshold || dy > threshold || dw > threshold || dh > threshold {
                    let label = if let Some(idx) = pn.source_index {
                        format!("src-{}", idx)
                    } else {
                        pn.id.clone()
                    };
                    diffs.push((label, dx, dy, dw, dh, lb, rb));
                }
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
    let unmatched = secondary_pool.len();
    if unmatched > 0 {
        println!("{} secondary nodes unmatched.", unmatched);
    }
    Ok(())
}

fn build_from_layout_json(path: &str) -> Result<LayoutCapture> {
    let text = std::fs::read_to_string(path)?;
    let layout: crate::layout::LayoutData = serde_json::from_str(&text)?;
    let nodes: Vec<Node> = layout
        .elements
        .iter()
        .map(|e| Node {
            id: format!("elem-{}", e.index),
            source_index: Some(e.index),
            backend_node_id: None,
            node_type: "element".into(),
            tag: Some(e.tag.clone()),
            classes: e.classes.clone(),
            pseudo: None,
            text: e.text_content.clone(),
            box_model: Some(Rect {
                x: e.x as f64,
                y: e.y as f64,
                w: e.width as f64,
                h: e.height as f64,
            }),
            client_rects: vec![],
            inline_text_boxes: vec![],
            styles: serde_json::Map::new(),
        })
        .collect();

    Ok(LayoutCapture {
        metadata: Metadata {
            url: path.to_string(),
            viewport: Viewport {
                w: layout.metadata.viewport.width,
                h: layout.metadata.viewport.height,
                dpr: layout.metadata.viewport.device_pixel_ratio,
            },
            captured_at: chrono::Utc::now().to_rfc3339(),
            chrome: "layout-json".into(),
        },
        nodes,
    })
}
