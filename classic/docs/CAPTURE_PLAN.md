# Layout Capture & Diff Plan

Goal: add two Rust CLI commands to capture precise layout metadata from (a) the reference TodoMVC HTML and (b) our renderer, then diff them.

## Extraction strategy
- Use Chromium DevTools Protocol (chromiumoxide) with `DOMSnapshot.captureSnapshot`, requesting layout + inlineTextBoxes + pseudo-elements and a minimal computed-style set (font-size, font-family, font-weight, line-height, color, background-color, border-*, border-radius, box-shadow, opacity, visibility, z-index).
- Fallback JS snippet for any node missing boxes: `getBoundingClientRect()` and `Range.getClientRects()` for text nodes.
- IDs: stable path (tag + sibling index) + backendNodeId + pseudo tag; store nodeType and text content for text nodes to keep ordering deterministic.

## Schema (new)
```
{
  "metadata": { "url": "...", "viewport": {"w":700,"h":700,"dpr":1}, "captured_at": "...", "chrome": "..." },
  "nodes": [
    {
      "id": "body/0/div.todoapp/1",
      "backend_node_id": 123,
      "node_type": "element|text|pseudo",
      "tag": "div",
      "classes": ["todoapp"],
      "pseudo": "before|after|null",
      "text": "Buy groceries",
      "box": {"x":..,"y":..,"w":..,"h":..},
      "client_rects": [ {..}, ... ],
      "inline_text_boxes": [ {..}, ... ],
      "styles": { "font_size": "24px", ... }
    }
  ]
}
```

## CLI commands (tools crate)
- `capture-reference [--url | --file reference/html/todomvc_populated.html] [--out reference/layouts/layout_precise_reference.json] [--port auto] [--headed] [--chrome-path PATH]`
  - Serve `reference/` on a free port; viewport 700×700, DPR=1; Chrome with WebGPU flags.
- `capture-renderer [--url http://localhost:8000] [--out reference/layouts/layout_precise_renderer.json] [--port override] [--headed] [--chrome-path PATH]`
  - Assume `wasm-start` running; optionally serve `web/` if needed.
- `diff-layouts --a reference/layouts/layout_precise_reference.json --b reference/layouts/layout_precise_renderer.json [--threshold 0.1]`
  - Git-like diff per node for x/y/w/h (and any other overlapping fields).

## Chrome flags
Use from docs/CHROME_SETUP.md:
`--enable-unsafe-webgpu --enable-webgpu-developer-features --enable-features=Vulkan,VulkanFromANGLE --enable-vulkan --use-angle=vulkan --disable-software-rasterizer --ozone-platform=x11 --headless=new --remote-debugging-port <free> --user-data-dir /tmp/raybox-cdp`

## Validation
- Run `capture-reference` headless and headed; diff the two outputs to confirm parity. Adjust if differences appear.

## Ports
- Auto-pick free port (start at 8000, fall back upward) to avoid conflicts with 3000/8080/etc.
