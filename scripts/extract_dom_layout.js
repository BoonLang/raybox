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
 *   5. Paste into reference/layouts/layout.json
 */

function extractDOMLayout() {
  const elements = [];
  const allElements = document.querySelectorAll('*');

  // Get viewport info (note: devicePixelRatio kept as number for Rust f32 compatibility)
  const viewport = {
    width: window.innerWidth,
    height: window.innerHeight,
    devicePixelRatio: parseFloat(window.devicePixelRatio) || 1.0,
  };

  allElements.forEach((el, index) => {
    const rect = el.getBoundingClientRect();
    const computed = window.getComputedStyle(el);

    // Skip completely invisible elements
    if (rect.width === 0 && rect.height === 0) return;
    if (computed.visibility === 'hidden') return;
    if (computed.display === 'none') return;

    // Checkboxes may have opacity:0 (TodoMVC uses SVG background on label)
    // but renderer draws them as circles, so keep them
    const isCheckboxInput = el.tagName === 'INPUT' && el.type === 'checkbox';
    if (parseFloat(computed.opacity) === 0 && !isCheckboxInput) return;

    // Visual significance filtering - only extract elements that contribute visually
    const hasBg = computed.backgroundColor !== 'rgba(0, 0, 0, 0)'
               && computed.backgroundColor !== 'transparent';

    const hasBorder = (
      parseFloat(computed.borderTopWidth) > 0 ||
      parseFloat(computed.borderRightWidth) > 0 ||
      parseFloat(computed.borderBottomWidth) > 0 ||
      parseFloat(computed.borderLeftWidth) > 0
    );

    const hasShadow = computed.boxShadow !== 'none';

    // Direct text content (not inherited from children)
    const hasDirectText = Array.from(el.childNodes)
      .filter(n => n.nodeType === Node.TEXT_NODE)
      .some(n => n.textContent.trim().length > 0);

    // Interactive elements always included
    const isInteractive = ['INPUT', 'BUTTON', 'SELECT', 'TEXTAREA'].includes(el.tagName);

    // Root elements (html, body) always included for structure
    const isRoot = ['HTML', 'BODY'].includes(el.tagName);

    // Skip inline text styling elements (they duplicate parent text content)
    // These elements exist only to style portions of text, not as separate visual elements
    const isInlineTextStyling = ['STRONG', 'EM', 'B', 'I', 'U', 'S', 'MARK', 'SMALL', 'SUB', 'SUP'].includes(el.tagName);

    // Skip nested <a> tags inside text containers (they're part of the parent's text)
    // But keep <a> tags inside <li> that are navigation/filter buttons
    const isNestedLink = el.tagName === 'A' && el.parentElement &&
      ['P', 'SPAN'].includes(el.parentElement.tagName);

    if (isInlineTextStyling || isNestedLink) {
      return;
    }

    // Skip if no visual significance (isCheckboxInput defined earlier for opacity check)
    if (!hasBg && !hasBorder && !hasShadow && !hasDirectText && !isInteractive && !isCheckboxInput && !isRoot) {
      return;
    }

    // Get text content (truncated for readability)
    let textContent = '';

    // Check if this is a "leaf" text container (has inline children but no block children)
    // Leaf text containers should use full textContent (e.g., "<span><strong>3</strong> items left</span>")
    // Containers with block children should only use direct text nodes
    const hasBlockChildren = Array.from(el.children).some(child => {
      const display = window.getComputedStyle(child).display;
      return display === 'block' || display === 'flex' || display === 'grid' ||
             display === 'list-item' || display === 'table';
    });

    if (hasBlockChildren) {
      // For containers with block children, only get direct text nodes
      textContent = Array.from(el.childNodes)
        .filter(node => node.nodeType === Node.TEXT_NODE)
        .map(node => node.textContent.trim())
        .filter(t => t.length > 0)
        .join(' ');
    } else {
      // For leaf text containers, use full textContent (includes inline children)
      textContent = el.textContent?.trim() || '';
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

      // Note: left/top/right/bottom from getBoundingClientRect are NOT included
      // because they conflict with CSS positioning properties (which are strings like "10px")
      // Use x/y/width/height instead for element positioning

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
      borderBottom: computed.borderBottom,
      borderTop: computed.borderTop,
      borderLeft: computed.borderLeft,
      borderRight: computed.borderRight,
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

      // State (use explicit boolean check to preserve false vs null distinction)
      checked: el.type === 'checkbox' ? el.checked : null,
      disabled: el.disabled === true ? true : null,

      // Visibility
      // Note: checkboxes have opacity:0 in CSS but renderer draws them as circles,
      // so we don't include opacity for checkbox inputs
      visibility: computed.visibility,
      opacity: isCheckboxInput ? null : computed.opacity,
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
      timestamp: new Date().toISOString(),
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
