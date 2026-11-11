export type IndexEntry = {
  frame: string;
  id: string;
  label: string;
  src: string;
};

let frameId = "";
let focusId = "";
const entries: IndexEntry[] = [];

export function resetCatalog(): void {
  entries.length = 0;
  frameId = "";
  focusId = "";
}

export function beginFrame(id: string): void {
  frameId = id;
  focusId = "";
}

export function setFocus(id: string | undefined): void {
  focusId = id ?? "";
}

export function catalog(): IndexEntry[] {
  return entries;
}

export function esc(value: string): string {
  return value.replace(/[&<>"]/g, (ch) => {
    if (ch === "&") return "&amp;";
    if (ch === "<") return "&lt;";
    if (ch === ">") return "&gt;";
    return "&quot;";
  });
}

export function el(
  id: string,
  label: string,
  src: string,
  html: string,
  opts?: { tag?: string; class?: string; extra?: string },
): string {
  if (!frameId) throw new Error(`element ${id} rendered outside a frame`);
  entries.push({ frame: frameId, id, label, src });
  const tag = opts?.tag ?? "div";
  const focused = focusId === id;
  const ring = focused ? ` data-focus="true"` : "";
  const cls = opts?.class ? ` class="${opts.class}"` : "";
  const extra = opts?.extra ? ` ${opts.extra}` : "";
  return `<${tag} data-el="${esc(id)}" data-label="${esc(label)}" data-src="${esc(src)}"${cls}${ring}${extra}>${html}</${tag}>`;
}
