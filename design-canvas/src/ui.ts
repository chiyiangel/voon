import { el, esc } from "./catalog";
import { icon } from "./icons";
import { A, type Album, type Source, type Track, STATS } from "./data";

export const SRC = {
  window: "crates/voon-app/src/ui/window.rs",
  sidebar: "crates/voon-app/src/ui/sidebar.rs",
  toolbar: "crates/voon-app/src/ui/toolbar.rs",
  grid: "crates/voon-app/src/ui/album_grid.rs",
  list: "crates/voon-app/src/ui/album_list.rs",
  detail: "crates/voon-app/src/ui/album_detail.rs",
  edit: "crates/voon-app/src/ui/edit_sheet.rs",
  merge: "crates/voon-app/src/ui/merge_sheet.rs",
  cover: "crates/voon-app/src/ui/cover_picker.rs",
  collection: "crates/voon-app/src/ui/collection_view.rs",
  composers: "crates/voon-app/src/ui/composer_view.rs",
  facets: "crates/voon-app/src/ui/facet_view.rs",
  tags: "crates/voon-app/src/ui/tags_view.rs",
  inbox: "crates/voon-app/src/ui/inbox_view.rs",
  onboarding: "crates/voon-app/src/ui/onboarding.rs",
  scan: "crates/voon-app/src/ui/scan_banner.rs",
  offline: "crates/voon-app/src/ui/offline_banner.rs",
  settings: "crates/voon-app/src/ui/settings.rs",
};

export const SRC_LABEL: Record<Source, string> = { tag: "标签", cue: "CUE", dir: "目录名", manual: "手动" };

export function srcBadge(id: string, s: Source, src = SRC.detail): string {
  const why = { tag: "来自文件内嵌标签", cue: "来自 CUE 分轨表", dir: "从目录名解析", manual: "你手动改过" }[s];
  return el(id, `来源：${SRC_LABEL[s]}（${why}）`, src, SRC_LABEL[s], { tag: "span", class: `src ${s}` });
}

export function btn(id: string, label: string, src: string, kind = "", glyph?: string): string {
  return el(id, label, src, `${glyph ? icon(glyph, 14) : ""}<span>${esc(label)}</span>`, { class: `btn ${kind}`.trim(), tag: "span" });
}

export function iconBtn(id: string, label: string, src: string, glyph: string): string {
  return el(id, label, src, icon(glyph, 16), { class: "icon-btn", tag: "span", extra: `title="${esc(label)}"` });
}

function fmtChip(a: Album): string {
  const cls = a.kind === "DSD" ? "fmt dsd" : /24|DXD/.test(a.spec) ? "fmt hr" : "fmt";
  const text = a.kind === "DSD" ? a.spec : a.spec === "16/44.1" ? a.format : a.spec.replace("DXD 352.8k", "DXD");
  return `<span class="${cls}">${esc(text)}</span>`;
}

export function coverArt(a: Album, opts: { chip?: boolean; size?: number } = {}): string {
  const style = opts.size ? `width:${opts.size}px;` : "";
  if (a.noCover) {
    return `<div class="cover none" style="${style}"><span class="ini">无封面</span></div>`;
  }
  const h2 = a.hue2 ?? a.hue + 30;
  const bg = `background:linear-gradient(145deg, hsl(${a.hue} 62% 52%), hsl(${h2} 58% 30%));`;
  const fs = opts.size && opts.size < 70 ? "font-size:14px" : a.ini.length > 4 ? "font-size:20px" : "";
  return `<div class="cover" style="${style}${bg}"><span class="ring"></span>${opts.chip ? `<span class="chip">${fmtChip(a)}</span>` : ""}<span class="ini" style="${fs}">${esc(a.ini)}</span></div>`;
}

export function lights(off = false): string {
  return el("window.traffic-lights", "关闭、最小化、缩放", SRC.window, "<i></i><i></i><i></i>", { class: `lights${off ? " off" : ""}` });
}

export type NavKey = "library" | "collections" | "composers" | "facets" | "tags" | "stacks" | "inbox" | "none";

export function sidebar(on: NavKey, opts: { empty?: boolean; offline?: boolean; scanning?: boolean; inbox?: number } = {}): string {
  const dim = opts.empty ? " dim" : "";
  const item = (key: NavKey, label: string, glyph: string, right = "") =>
    el(`sidebar.${key}`, label, SRC.sidebar, `${icon(glyph, 16)}<span>${esc(label)}</span>${right}`, { class: `nav${on === key ? " on" : ""}${dim}` });
  const count = (n: string | number) => (opts.empty ? "" : `<span class="count">${n}</span>`);
  const inboxN = opts.inbox ?? STATS.inbox;
  const sub = (id: string, label: string, color: string, n: number) =>
    el(`sidebar.${id}`, label, SRC.sidebar, `<span class="dot" style="background:${color}"></span><span>${esc(label)}</span><span class="count">${n}</span>`, { class: "nav sub" });
  const foot = opts.empty
    ? `<span class="led off"></span><span><b>尚未添加音乐库</b>添加后开始建立本地索引</span>`
    : opts.offline
      ? `<span class="led off"></span><span><b>NAS1 未挂载</b>使用本地索引 · ${STATS.lastScan}</span>`
      : opts.scanning
        ? `<span class="led" style="background:var(--accent)"></span><span><b>NAS1 · 正在扫描</b>/Volumes/NAS1/Music</span>`
        : `<span class="led"></span><span><b>NAS1 · 已连接</b>${STATS.albums} 张专辑 · ${STATS.size}</span>`;
  return `<aside class="sb">
    ${lights()}
    <div class="sb-group">资料库</div>
    ${item("library", "音乐库", "library", count(opts.scanning ? 214 : STATS.albums))}
    ${item("collections", "套装", "box", count(opts.scanning ? 2 : STATS.collections))}
    ${item("composers", "作曲家", "composer", count(opts.scanning ? 31 : 112))}
    ${item("facets", "分面", "facet")}
    <div class="sb-group">整理</div>
    ${item("tags", "标签", "tag", count(opts.empty ? "" : 8))}
    ${item("stacks", "合集", "stack", count(opts.empty ? "" : 5))}
    ${item("inbox", "待处理", "inbox", opts.empty ? "" : `<span class="badge">${opts.scanning ? 6 : inboxN}</span>`)}
    ${opts.empty ? "" : `<div class="sb-group">合集</div>
    ${sub("stack.night", "睡前小提琴", "#b4532a", 24)}
    ${sub("stack.dsd", "DSD 试音碟", "#7a3fb0", 17)}
    ${sub("stack.car", "车载 · 粤语", "#2f6fdc", 31)}`}
    ${el("sidebar.library-status", opts.offline ? "NAS1 未挂载" : opts.empty ? "尚未添加音乐库" : "NAS1 · 已连接", SRC.sidebar, foot, { class: "sb-foot" })}
  </aside>`;
}

export function toolbar(title: string, opts: { sub?: string; view?: "grid" | "list" | null; search?: string; extra?: string; rescan?: boolean } = {}): string {
  const view =
    opts.view === null || opts.view === undefined
      ? ""
      : el(
          "toolbar.view",
          opts.view === "grid" ? "视图：网格（可切换为列表）" : "视图：列表（可切换为网格）",
          SRC.toolbar,
          `<span class="${opts.view === "grid" ? "on" : ""}">${icon("grid", 14)}</span><span class="${opts.view === "list" ? "on" : ""}">${icon("list", 14)}</span>`,
          { class: "seg" },
        );
  return `<div class="tb">
    <h1>${esc(title)}</h1>${opts.sub ? `<span class="sub">${esc(opts.sub)}</span>` : ""}
    <span class="grow"></span>
    ${opts.extra ?? ""}
    ${view}
    ${opts.view ? el("toolbar.sort", "排序：最近添加", SRC.toolbar, `${icon("sort", 14)}<span>最近添加</span>`, { class: "btn quiet", tag: "span" }) : ""}
    ${el("toolbar.search", "搜索专辑、艺术家、作曲家", SRC.toolbar, `${icon("search", 14)}<span>${esc(opts.search ?? "搜索专辑、艺术家、作曲家")}</span>`, { class: "search" })}
    ${opts.rescan === false ? "" : iconBtn("toolbar.rescan", "重新扫描（增量）", SRC.toolbar, "refresh")}
  </div>`;
}

export function card(a: Album, opts: { on?: boolean; idPrefix?: string; isNew?: boolean } = {}): string {
  const p = opts.idPrefix ?? "grid";
  return el(
    `${p}.album.${a.id}`,
    a.title,
    SRC.grid,
    `${coverArt(a, { chip: true })}<div class="t">${esc(a.title)}</div><div class="a">${esc(a.artist)}</div>`,
    { class: `card${opts.on ? " on" : ""}${opts.isNew ? " new" : ""}` },
  );
}

export function ghost(i: number): string {
  return el(`grid.placeholder.${i}`, "扫描中占位", SRC.grid, `<div class="cover"></div><div class="t">　</div><div class="a">　</div>`, { class: "card ghost" });
}

export function grid(ids: string[], selected?: string, opts: { idPrefix?: string; ghosts?: number; cols?: number; newFrom?: number } = {}): string {
  const cards = ids.map((id, i) => card(A[id], { on: id === selected, idPrefix: opts.idPrefix, isNew: opts.newFrom !== undefined && i >= opts.newFrom })).join("");
  const g = Array.from({ length: opts.ghosts ?? 0 }, (_, i) => ghost(i)).join("");
  const style = opts.cols ? ` style="grid-template-columns:repeat(${opts.cols}, minmax(0,1fr))"` : "";
  return `<div class="grid"${style}>${cards}${g}</div>`;
}

export function field(id: string, k: string, v: string, s: Source | null, src = SRC.detail): string {
  return el(
    `detail.field.${id}`,
    `${k}：${v}`,
    src,
    `<span class="k">${esc(k)}</span><span class="v">${esc(v)}</span>${s ? srcBadge(`detail.field.${id}.source`, s, src) : "<span></span>"}`,
    { class: "field" },
  );
}

export function trackRow(prefix: string, t: Track, virtual = false): string {
  return el(
    `${prefix}.${t.n}`,
    t.name,
    SRC.detail,
    `<span class="n">${esc(t.n)}</span><span class="name">${esc(t.name)}</span><span class="d">${esc(t.d)}</span>`,
    { class: `trk${virtual ? " virtual" : ""}` },
  );
}

export function detailShell(a: Album, body: string, opts: { offline?: boolean; meta?: string } = {}): string {
  const meta = opts.meta ?? `${a.year ? a.year + " · " : ""}${a.format} ${a.spec} · ${a.discs} 碟 · ${a.tracks} 轨 · ${a.size}`;
  return `<aside class="dt">
    <div class="dt-head">
      <div class="dt-top">
        ${el("detail.cover", "专辑封面（点击更换）", SRC.detail, coverArt(a, { size: 120 }))}
        <div style="min-width:0;display:flex;flex-direction:column;gap:4px">
          ${el("detail.title", a.title, SRC.detail, esc(a.title), { class: "dt-title" })}
          ${el("detail.artist", a.artist, SRC.detail, esc(a.artist), { class: "dt-artist" })}
          ${el("detail.meta", meta, SRC.detail, esc(meta), { class: "dt-meta" })}
        </div>
      </div>
      <div class="dt-actions">
        ${btn("detail.edit", "编辑信息", SRC.detail, "", "edit")}
        ${btn("detail.open", "用默认播放器打开", SRC.detail, opts.offline ? "disabled" : "", "open")}
        ${iconBtn("detail.more", "更多：合并、拆分、选择封面、在访达中显示", SRC.detail, "dots")}
      </div>
    </div>
    ${body}
  </aside>`;
}
