pub const JS_COLLECT_RECTS: &str = r##"
(() => {
  function pathFor(node) {
    if (!node || node === document) return "root";
    const parent = node.parentNode;
    const parentPath = pathFor(parent);
    const tag =
      node.nodeType === Node.TEXT_NODE
        ? "#text"
        : (node.tagName || "node").toLowerCase();
    const siblings = Array.from(parent.childNodes).filter(
      (n) =>
        n.nodeType === node.nodeType &&
        (n.tagName === node.tagName || n.nodeType === Node.TEXT_NODE)
    );
    const idx = siblings.indexOf(node);
    return `${parentPath}/${tag}[${idx >= 0 ? idx : 0}]`;
  }

  function rectToObj(r) {
    return { x: r.x, y: r.y, w: r.width, h: r.height };
  }

  const results = {};
  function visit(node) {
    const p = pathFor(node);
    let rect = null;
    try {
      if (node.nodeType === Node.TEXT_NODE) {
        const range = document.createRange();
        range.selectNodeContents(node);
        const clientRects = range.getClientRects();
        if (clientRects.length > 0) {
          rect = rectToObj(clientRects[0]);
        }
      } else if (node.getBoundingClientRect) {
        rect = rectToObj(node.getBoundingClientRect());
      }
    } catch (e) {}
    if (rect) {
      results[p] = rect;
    }
    for (const child of node.childNodes) {
      visit(child);
    }
  }

  visit(document.documentElement);
  return results;
})();
"##;
