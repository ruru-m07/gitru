// ============================================================================
// HAST (Hypertext Abstract Syntax Tree) Utilities
// Work with AST objects instead of strings - convert to HTML once at the end
// ============================================================================

/**
 * HAST Element node type
 */
export interface HASTElement {
  type: "element";
  tagName: string;
  properties: Record<string, string | number | boolean | undefined>;
  children: HASTNode[];
}

/**
 * HAST Text node type
 */
export interface HASTText {
  type: "text";
  value: string;
}

/**
 * Union of all HAST node types
 */
export type HASTNode = HASTElement | HASTText;

// ============================================================================
// Node Creation
// ============================================================================

/**
 * Create a text node
 */
export function createText(value: string): HASTText {
  return { type: "text", value };
}

/**
 * Create an element node
 */
export function createElement(
  tagName: string,
  properties: HASTElement["properties"] = {},
  children: HASTNode[] = [],
): HASTElement {
  return {
    type: "element",
    tagName,
    properties,
    children,
  };
}

/**
 * Create a span element (most common for syntax highlighting)
 */
export function createSpan(
  properties: HASTElement["properties"] = {},
  children: HASTNode[] = [],
): HASTElement {
  return createElement("span", properties, children);
}

/**
 * Create a div element
 */
export function createDiv(
  properties: HASTElement["properties"] = {},
  children: HASTNode[] = [],
): HASTElement {
  return createElement("div", properties, children);
}

// ============================================================================
// HTML Conversion (done once at the end)
// ============================================================================

const VOID_ELEMENTS = new Set([
  "area",
  "base",
  "br",
  "col",
  "embed",
  "hr",
  "img",
  "input",
  "link",
  "meta",
  "param",
  "source",
  "track",
  "wbr",
]);

/**
 * Convert a HAST node to an HTML string
 * This should only be called once at render time
 */
export function toHtml(node: HASTNode): string {
  if (node.type === "text") {
    return escapeHtmlText(node.value);
  }

  const { tagName, properties, children } = node;

  // Build attributes string
  let attrs = "";
  for (const [key, value] of Object.entries(properties)) {
    if (value === undefined || value === false) continue;
    if (value === true) {
      attrs += ` ${key}`;
    } else {
      attrs += ` ${key}="${escapeAttr(String(value))}"`;
    }
  }

  // Void elements don't have closing tags
  if (VOID_ELEMENTS.has(tagName)) {
    return `<${tagName}${attrs}>`;
  }

  // Render children
  const childrenHtml = children.map(toHtml).join("");

  return `<${tagName}${attrs}>${childrenHtml}</${tagName}>`;
}

/**
 * Convert an array of HAST nodes to HTML
 */
export function toHtmlArray(nodes: HASTNode[]): string {
  return nodes.map(toHtml).join("");
}

// ============================================================================
// Escape Utilities
// ============================================================================

/**
 * Escape HTML special characters in text content
 * Note: Named differently from common.ts to avoid conflict
 */
function escapeHtmlText(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

/**
 * Escape HTML attribute values
 */
export function escapeAttr(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

// ============================================================================
// Node Manipulation
// ============================================================================

/**
 * Append children to an element
 */
export function appendChild(parent: HASTElement, child: HASTNode): void {
  parent.children.push(child);
}

/**
 * Append multiple children to an element
 */
export function appendChildren(
  parent: HASTElement,
  children: HASTNode[],
): void {
  parent.children.push(...children);
}

/**
 * Clone a node (shallow for elements, full for text)
 */
export function cloneNode(node: HASTNode): HASTNode {
  if (node.type === "text") {
    return { type: "text", value: node.value };
  }

  return {
    type: "element",
    tagName: node.tagName,
    properties: { ...node.properties },
    children: [...node.children],
  };
}

/**
 * Deep clone a node
 */
export function deepCloneNode(node: HASTNode): HASTNode {
  if (node.type === "text") {
    return { type: "text", value: node.value };
  }

  return {
    type: "element",
    tagName: node.tagName,
    properties: { ...node.properties },
    children: node.children.map(deepCloneNode),
  };
}

// ============================================================================
// Line-specific utilities
// ============================================================================

/**
 * Wrap HAST nodes in an inline diff marker span
 */
export function wrapInInlineDiffMarker(nodes: HASTNode[]): HASTElement {
  return createSpan({ "data-diff-inline": "" }, nodes);
}

/**
 * Create a line content element from HAST nodes
 */
export function createLineContent(nodes: HASTNode[]): HASTElement {
  return createDiv(
    { class: "diff-line-content", "data-column-content": "" },
    nodes,
  );
}

/**
 * Check if a node is an element
 */
export function isElement(node: HASTNode): node is HASTElement {
  return node.type === "element";
}

/**
 * Check if a node is a text node
 */
export function isText(node: HASTNode): node is HASTText {
  return node.type === "text";
}

// ============================================================================
// Transferable conversion (for worker communication)
// ============================================================================

/**
 * HAST nodes are plain objects and can be transferred directly via postMessage
 * No conversion needed - they're already structured cloneable
 */
export type TransferableHAST = HASTNode;

/**
 * For very large ASTs, we can use a more compact representation
 * [type, tagName/value, properties, children] as tuple
 */
export type CompactHAST =
  | ["t", string] // text node: ["t", value]
  | ["e", string, Record<string, unknown>, CompactHAST[]]; // element: ["e", tagName, props, children]

export function toCompact(node: HASTNode): CompactHAST {
  if (node.type === "text") {
    return ["t", node.value];
  }
  return ["e", node.tagName, node.properties, node.children.map(toCompact)];
}

export function fromCompact(compact: CompactHAST): HASTNode {
  if (compact[0] === "t") {
    return { type: "text", value: compact[1] };
  }
  return {
    type: "element",
    tagName: compact[1],
    properties: compact[2] as HASTElement["properties"],
    children: compact[3].map(fromCompact),
  };
}
