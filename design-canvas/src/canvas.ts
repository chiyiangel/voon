import productCss from "./product.css?raw";

const viewport = document.querySelector<HTMLElement>("#viewport")!;
const world = document.querySelector<HTMLElement>("#world")!;
const zoomLabel = document.querySelector<HTMLElement>("#zoom-label")!;
const toast = document.querySelector<HTMLElement>("#toast")!;
const jump = document.querySelector<HTMLSelectElement>("#jump")!;

type Mode = "select" | "pan";

const camera = { x: 40, y: 24, z: 0.25 };
let mode: Mode = "select";
let space = false;
let dragging = false;
let moved = false;
let start = { x: 0, y: 0, cx: 0, cy: 0 };
let armed: HTMLElement | null = null;
let picked: HTMLElement | null = null;
let toastTimer = 0;
let hashTimer = 0;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function apply(): void {
  world.style.transform = `translate(${camera.x}px, ${camera.y}px) scale(${camera.z})`;
  zoomLabel.textContent = `${Math.round(camera.z * 100)}%`;
}

function scheduleHash(frame?: string): void {
  window.clearTimeout(hashTimer);
  hashTimer = window.setTimeout(() => writeHash(frame), 120);
}

function writeHash(frame?: string): void {
  const params = new URLSearchParams();
  const current = frame ?? new URLSearchParams(location.hash.slice(1)).get("f") ?? "";
  if (current) params.set("f", current);
  params.set("z", camera.z.toFixed(3));
  params.set("x", camera.x.toFixed(1));
  params.set("y", camera.y.toFixed(1));
  history.replaceState(null, "", `#${params.toString()}`);
}

function worldBox(el: HTMLElement): { x: number; y: number; w: number; h: number } {
  const node = el.getBoundingClientRect();
  const origin = world.getBoundingClientRect();
  return {
    x: (node.left - origin.left) / camera.z,
    y: (node.top - origin.top) / camera.z,
    w: node.width / camera.z,
    h: node.height / camera.z,
  };
}

export function zoomAt(clientX: number, clientY: number, factor: number): void {
  const next = clamp(camera.z * factor, 0.04, 2.5);
  const wx = (clientX - camera.x) / camera.z;
  const wy = (clientY - camera.y) / camera.z;
  camera.z = next;
  camera.x = clientX - wx * next;
  camera.y = clientY - wy * next;
  apply();
  scheduleHash();
}

export function zoomToFit(): void {
  const pad = 48;
  const bounds = world.getBoundingClientRect();
  const width = bounds.width / camera.z;
  const height = bounds.height / camera.z;
  const next = clamp(
    Math.min((viewport.clientWidth - pad) / width, (viewport.clientHeight - pad) / height),
    0.04,
    1,
  );
  camera.z = next;
  camera.x = (viewport.clientWidth - width * next) / 2;
  camera.y = (viewport.clientHeight - height * next) / 2;
  apply();
  scheduleHash();
}

export function zoomTo100(): void {
  zoomAt(viewport.clientWidth / 2, viewport.clientHeight / 2, 1 / camera.z);
}

export function focusFrame(id: string): void {
  const frame = document.querySelector<HTMLElement>(`[data-frame="${CSS.escape(id)}"]`);
  if (!frame) return;
  const box = worldBox(frame);
  const pad = 56;
  const next = clamp(
    Math.min(
      (viewport.clientWidth - pad * 2) / box.w,
      (viewport.clientHeight - pad * 2) / box.h,
    ),
    0.12,
    1,
  );
  camera.z = next;
  camera.x = (viewport.clientWidth - box.w * next) / 2 - box.x * next;
  camera.y = (viewport.clientHeight - box.h * next) / 2 - box.y * next;
  apply();
  writeHash(id);
  if (jump.value !== id) jump.value = id;
}

function setMode(next: Mode): void {
  mode = next;
  viewport.classList.toggle("is-pan", next === "pan");
  document.querySelector("#mode-select")?.classList.toggle("is-on", next === "select");
  document.querySelector("#mode-pan")?.classList.toggle("is-on", next === "pan");
  if (next === "pan") clearHot();
}

function clearHot(): void {
  document.querySelectorAll(".is-hot").forEach((node) => node.classList.remove("is-hot"));
}

function showToast(full: string, id: string): void {
  toast.hidden = false;
  toast.dataset.copied = full;
  toast.textContent = `已复制 · ${id}`;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.hidden = true;
  }, 2200);
}

async function copyElement(node: HTMLElement): Promise<void> {
  const frame = node.closest<HTMLElement>("[data-frame]")?.dataset.frame ?? "";
  const id = node.dataset.el ?? "";
  const label = (node.dataset.label ?? "").replaceAll('"', '\\"');
  const src = node.dataset.src ?? "";
  const text = `[voon-design] frame=${frame} > el=${id} | label="${label}"${src ? ` | src=${src}` : ""}`;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    document.body.append(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
  (window as unknown as { __voonCopied?: string }).__voonCopied = text;
  if (picked) picked.classList.remove("is-picked");
  node.classList.add("is-picked");
  picked = node;
  showToast(text, id);
}

function wireFrame(frame: HTMLElement): void {
  frame.querySelector<HTMLButtonElement>("[data-act='expand']")?.addEventListener("click", (event) => {
    event.stopPropagation();
    focusFrame(frame.dataset.frame ?? "");
  });
  frame.querySelector<HTMLButtonElement>("[data-act='play']")?.addEventListener("click", (event) => {
    event.stopPropagation();
    frame.classList.remove("is-playing");
    void frame.offsetWidth;
    frame.classList.add("is-playing");
  });
  frame.querySelector<HTMLButtonElement>("[data-act='download']")?.addEventListener("click", (event) => {
    event.stopPropagation();
    void downloadFrame(frame);
  });
}

async function downloadFrame(frame: HTMLElement): Promise<void> {
  const css = productCss;
  const stage = frame.querySelector(".c-stage")?.innerHTML ?? "";
  const title = frame.querySelector("h3")?.textContent ?? frame.dataset.frame ?? "frame";
  const html = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>${title}</title><style>${css}</style><body style="margin:24px;background:#e6e4df">${stage}</body></html>`;
  const blob = new Blob([html], { type: "text/html" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `${frame.dataset.frame ?? "frame"}.html`;
  link.click();
  URL.revokeObjectURL(link.href);
}

function readHash(): void {
  const raw = location.hash.replace(/^#/, "");
  if (!raw) {
    zoomToFit();
    return;
  }
  const params = new URLSearchParams(raw.includes("=") ? raw : `f=${raw}`);
  const frame = params.get("f");
  const z = Number(params.get("z"));
  const x = Number(params.get("x"));
  const y = Number(params.get("y"));
  if (Number.isFinite(z) && Number.isFinite(x) && Number.isFinite(y)) {
    camera.z = clamp(z, 0.04, 2.5);
    camera.x = x;
    camera.y = y;
    apply();
    if (frame && jump.value !== frame) jump.value = frame;
    return;
  }
  if (frame) focusFrame(frame);
  else zoomToFit();
}

export function mountCanvas(frameIds: { id: string; number: string; title: string; section: string }[]): void {
  document.querySelectorAll<HTMLElement>("[data-frame]").forEach(wireFrame);
  const groups = new Map<string, typeof frameIds>();
  for (const frame of frameIds) {
    const list = groups.get(frame.section) ?? [];
    list.push(frame);
    groups.set(frame.section, list);
  }
  jump.innerHTML = `<option value="">帧…</option>`;
  for (const [section, frames] of groups) {
    const group = document.createElement("optgroup");
    group.label = section;
    for (const frame of frames) {
      const option = document.createElement("option");
      option.value = frame.id;
      option.textContent = `${frame.number} · ${frame.title}`;
      group.append(option);
    }
    jump.append(group);
  }
  jump.addEventListener("change", () => {
    if (jump.value) focusFrame(jump.value);
  });
  document.querySelector("#mode-select")?.addEventListener("click", () => setMode("select"));
  document.querySelector("#mode-pan")?.addEventListener("click", () => setMode("pan"));
  document.querySelector("#zoom-fit")?.addEventListener("click", () => zoomToFit());
  document.querySelector("#zoom-100")?.addEventListener("click", () => zoomTo100());

  viewport.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      if (event.ctrlKey || event.metaKey) {
        zoomAt(event.clientX, event.clientY, Math.exp(-event.deltaY * 0.0016));
      } else {
        camera.x -= event.deltaX;
        camera.y -= event.deltaY;
        apply();
        scheduleHash();
      }
    },
    { passive: false },
  );

  viewport.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 && event.button !== 1) return;
    const target = event.target as HTMLElement;
    if (target.closest("#hud, .c-actions, #toast")) return;
    const element = mode === "select" ? target.closest<HTMLElement>("[data-el]") : null;
    const forcePan = event.button === 1 || space || mode === "pan";
    dragging = true;
    moved = false;
    armed = forcePan ? null : element;
    start = { x: event.clientX, y: event.clientY, cx: camera.x, cy: camera.y };
    if (forcePan || !element) viewport.classList.add("is-dragging");
    viewport.setPointerCapture(event.pointerId);
  });

  viewport.addEventListener("pointermove", (event) => {
    if (!dragging) {
      if (mode !== "select") return;
      clearHot();
      const element = (event.target as HTMLElement).closest<HTMLElement>("[data-el]");
      element?.classList.add("is-hot");
      return;
    }
    const dx = event.clientX - start.x;
    const dy = event.clientY - start.y;
    if (!moved && Math.hypot(dx, dy) > 4) {
      moved = true;
      armed = null;
      viewport.classList.add("is-dragging");
      clearHot();
    }
    if (moved) {
      camera.x = start.cx + dx;
      camera.y = start.cy + dy;
      apply();
    }
  });

  const endDrag = (event: PointerEvent) => {
    if (!dragging) return;
    dragging = false;
    viewport.classList.remove("is-dragging");
    if (!moved && armed) void copyElement(armed);
    else if (moved) scheduleHash();
    armed = null;
    if (viewport.hasPointerCapture(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
  };
  viewport.addEventListener("pointerup", endDrag);
  viewport.addEventListener("pointercancel", endDrag);

  window.addEventListener("keydown", (event) => {
    const typing = event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement;
    if (typing) return;
    if (event.key === "v" || event.key === "V") setMode("select");
    if (event.key === "h" || event.key === "H") setMode("pan");
    if (event.key === "0") zoomToFit();
    if (event.key === "1" && !event.metaKey && !event.ctrlKey) zoomTo100();
    if (event.key === " ") {
      event.preventDefault();
      space = true;
      viewport.classList.add("is-space");
    }
  });
  window.addEventListener("keyup", (event) => {
    if (event.key === " ") {
      space = false;
      viewport.classList.remove("is-space");
    }
  });

  apply();
  requestAnimationFrame(() => readHash());
}
