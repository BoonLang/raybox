/**
 * DOM Layout Extraction Script
 *
 * Extracts precise layout information from TodoMVC reference page.
 * Run this in Chrome DevTools console, then save the output.
 *
 * Usage:
 *   1. Open reference TodoMVC in Chrome
 *   2. Open DevTools console (F12)
 *   3. Copy-paste this entire script
 *   4. Run: copy(extractDOMLayout())
 *   5. Paste into reference/todomvc_dom_layout.json
 */

function extractDOMLayout() {
  const elements = [];
  const allElements = document.querySelectorAll('*');

  // Get viewport info
  const viewport = {
    width: window.innerWidth,
    height: window.innerHeight,
    devicePixelRatio: window.devicePixelRatio,
    timestamp: new Date().toISOString(),
  };

  allElements.forEach((el, index) => {
    const rect = el.getBoundingClientRect();
    const computed = window.getComputedStyle(el);

    // Skip elements with no size (unless they're important containers)
    const hasSize = rect.width > 0 || rect.height > 0;
    const isImportant = el.id || el.classList.length > 0 || el.tagName === 'INPUT';

    if (!hasSize && !isImportant) {
      return;
    }

    // Get text content (truncated for readability)
    let textContent = el.textContent?.trim() || '';
    if (el.childElementCount > 0) {
      // For containers, only get direct text nodes
      textContent = Array.from(el.childNodes)
        .filter(node => node.nodeType === Node.TEXT_NODE)
        .map(node => node.textContent.trim())
        .filter(t => t.length > 0)
        .join(' ');
    }
    textContent = textContent.substring(0, 100);

    const data = {
      // Identity
      index,
      tag: el.tagName.toLowerCase(),
      id: el.id || null,
      classes: Array.from(el.classList),

      // Position & Size (CSS pixels)
      x: Math.round(rect.x * 100) / 100,
      y: Math.round(rect.y * 100) / 100,
      width: Math.round(rect.width * 100) / 100,
      height: Math.round(rect.height * 100) / 100,

      // Position properties
      left: Math.round(rect.left * 100) / 100,
      top: Math.round(rect.top * 100) / 100,
      right: Math.round(rect.right * 100) / 100,
      bottom: Math.round(rect.bottom * 100) / 100,

      // Typography
      fontSize: computed.fontSize,
      fontFamily: computed.fontFamily,
      fontWeight: computed.fontWeight,
      fontStyle: computed.fontStyle,
      lineHeight: computed.lineHeight,
      letterSpacing: computed.letterSpacing,
      textAlign: computed.textAlign,
      textDecoration: computed.textDecoration,
      textTransform: computed.textTransform,
      whiteSpace: computed.whiteSpace,
      wordBreak: computed.wordBreak,

      // Box model
      display: computed.display,
      position: computed.position,
      flexDirection: computed.flexDirection,
      justifyContent: computed.justifyContent,
      alignItems: computed.alignItems,
      gap: computed.gap,

      padding: computed.padding,
      paddingTop: computed.paddingTop,
      paddingRight: computed.paddingRight,
      paddingBottom: computed.paddingBottom,
      paddingLeft: computed.paddingLeft,

      margin: computed.margin,
      marginTop: computed.marginTop,
      marginRight: computed.marginRight,
      marginBottom: computed.marginBottom,
      marginLeft: computed.marginLeft,

      border: computed.border,
      borderRadius: computed.borderRadius,

      // Dimensions
      minWidth: computed.minWidth,
      maxWidth: computed.maxWidth,
      minHeight: computed.minHeight,
      maxHeight: computed.maxHeight,

      // Colors (for V2)
      color: computed.color,
      backgroundColor: computed.backgroundColor,
      borderColor: computed.borderColor,

      // Shadows (for V2)
      boxShadow: computed.boxShadow,
      textShadow: computed.textShadow,

      // Content
      textContent: textContent,
      value: el.value || null,
      placeholder: el.placeholder || null,

      // State
      checked: el.checked || null,
      disabled: el.disabled || null,

      // Visibility
      visibility: computed.visibility,
      opacity: computed.opacity,
      zIndex: computed.zIndex,
    };

    elements.push(data);
  });

  // Return complete dataset
  return JSON.stringify({
    metadata: {
      url: window.location.href,
      title: document.title,
      userAgent: navigator.userAgent,
      timestamp: viewport.timestamp,
      viewport: viewport,
    },
    elements: elements,
    summary: {
      totalElements: elements.length,
      byTag: countByTag(elements),
      byClass: countByClass(elements),
    }
  }, null, 2);
}

function countByTag(elements) {
  const counts = {};
  elements.forEach(el => {
    counts[el.tag] = (counts[el.tag] || 0) + 1;
  });
  return counts;
}

function countByClass(elements) {
  const counts = {};
  elements.forEach(el => {
    el.classes.forEach(cls => {
      counts[cls] = (counts[cls] || 0) + 1;
    });
  });
  return counts;
}

// Export to global scope for console access
if (typeof window !== 'undefined') {
  window.extractDOMLayout = extractDOMLayout;
}

// If running in Node/automation
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { extractDOMLayout };
}

console.log('✅ extractDOMLayout() is ready!');
console.log('Run: copy(extractDOMLayout()) to copy to clipboard');
