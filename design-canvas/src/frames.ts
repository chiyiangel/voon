import { el, esc } from "./catalog";
import { icon } from "./icons";
import { A, BRANDENBURG_DISCS, COMPOSERS, GRID_MAIN, KLEIBER_TRACKS, ROOT, STATS, SUWANAI_TRACKS, type Album, type Source } from "./data";
import { SRC, btn, card, coverArt, detailShell, field, grid, iconBtn, lights, sidebar, srcBadge, toolbar, trackRow, type NavKey } from "./ui";

export type ThemeName = "light" | "dark";
export type FrameSpec = {
  id: string;
  number: string;
  title: string;
  section: string;
  caption: string;
  assumption?: string;
  width: number;
  themeable?: boolean;
  render: (theme: ThemeName) => string;
};

const W = 1280;
const H = 800;

function shell(theme: ThemeName, inner: string, w = W, h = H, cls = ""): string {
  return `<div class="product theme-${theme}"><div class="win ${cls}" style="width:${w}px;height:${h}px">${inner}</div></div>`;
}

function app(theme: ThemeName, nav: NavKey, main: string, detail = "", sbOpts: Parameters<typeof sidebar>[1] = {}, overlay = ""): string {
  return shell(theme, `${sidebar(nav, sbOpts)}<section class="main">${main}</section>${detail}${overlay}`);
}

/* ---------- detail bodies ---------- */

function suwanaiDetail(opts: { offline?: boolean } = {}): string {
  const a = A.suwanai;
  return detailShell(
    a,
    `<div class="dt-sec"><h4>专辑信息</h4>
      ${field("title", "标题", a.title, "tag")}
      ${field("artist", "艺术家", "諏訪内晶子 Akiko Suwanai", "tag")}
      ${field("composer", "作曲家", "J.S. Bach", "tag")}
      ${field("year", "年份", "2022", "dir")}
      ${field("genre", "流派", "古典", "tag")}
      ${field("tags", "标签", "睡前小提琴 · 独奏", "manual")}
    </div>
    <div class="dt-sec"><h4>曲目 · 碟 1 / 2</h4>
      ${SUWANAI_TRACKS.map((t) => trackRow("detail.track.d1", t)).join("")}
      <div class="more">… 碟 1 另 10 轨 · 碟 2 共 16 轨</div>
    </div>
    <div class="dt-sec"><h4>位置</h4>
      ${el("detail.path", a.dir, SRC.detail, `${esc(ROOT)}/<br>${esc(a.dir)}`, { class: "files" })}
    </div>`,
    opts,
  );
}

function brandenburgDetail(): string {
  const a = A.brandenburg;
  const discs = BRANDENBURG_DISCS.map(
    (d, i) => `
      ${el(`detail.disc.${i + 1}`, `${d.label} · ${d.files}`, SRC.detail, `${icon("disc", 14)}<span>${d.label}</span><span class="muted">${esc(d.files)}</span>${srcBadge(`detail.disc.${i + 1}.source`, "cue")}`, { class: "disc" })}
      ${d.tracks.map((t) => trackRow(`detail.track.d${i + 1}`, t, true)).join("")}
      ${d.more ? `<div class="more">${esc(d.more)}</div>` : ""}`,
  ).join("");
  return detailShell(
    a,
    `<div class="dt-sec"><h4>专辑信息</h4>
      ${field("title", "标题", "Bach: Brandenburg Concertos · Violin Concertos", "cue")}
      ${field("artist", "演奏者", "I Musici", "dir")}
      ${field("composer", "作曲家", "J.S. Bach", "cue")}
      ${field("catalog", "唱片编号", "438 317-2", "dir")}
      ${field("collection", "套装", "Philips Duo（193 张）", "dir")}
    </div>
    <div class="dt-sec"><h4>曲目 · 2 碟 · CUE 虚拟音轨</h4>${discs}</div>
    <div class="dt-sec"><h4>封面</h4>
      ${el("detail.cover-source", "封面来自 cover.jpg（目录图片）", SRC.detail, `<span class="k">来源</span><span class="v">cover.jpg · 1000×1000</span>${btn("detail.cover-change", "更换", SRC.detail, "quiet")}`, { class: "field" })}
    </div>`,
    { meta: "1993 · APE 16/44.1 · 2 碟 · 27 轨（虚拟）· 1.2 GB" },
  );
}

function kleiberDetail(): string {
  const a = A.kleiber;
  return detailShell(
    a,
    `<div class="dt-sec"><h4>古典信息</h4>
      ${field("composer", "作曲家", "Ludwig van Beethoven", "tag")}
      ${field("work", "作品", "Symphony No. 5, Op. 67 · No. 7, Op. 92", "tag")}
      ${field("conductor", "指挥", "Carlos Kleiber", "dir")}
      ${field("orchestra", "乐团", "Wiener Philharmoniker", "manual")}
      ${field("performer", "演奏者", "—", null)}
    </div>
    <div class="dt-sec"><h4>专辑信息</h4>
      ${field("title", "标题", "Beethoven: Symphony No. 5 & 7", "tag")}
      ${field("year", "年份", "1996", "dir")}
      ${field("genre", "流派", "古典 · 交响曲", "tag")}
    </div>
    <div class="dt-sec"><h4>曲目</h4>${KLEIBER_TRACKS.map((t) => trackRow("detail.track.d1", t)).join("")}</div>`,
  );
}

/* ---------- list view ---------- */

const LIST_ROWS = ["suwanai", "kv563", "brandenburg", "kleiber", "bachdsd", "aga", "evanescence", "emiri", "tongli", "lisiecki", "karajan9", "quiet", "guzheng", "mozartvn", "wukong", "langlang", "twelve", "vikingur", "jade", "minnaar"];

function listView(selected: string): string {
  const head = `<tr>
    <th style="width:25%" class="sorted">专辑 ${icon("down", 10)}</th><th style="width:16%">艺术家/演奏者</th><th style="width:12%">作曲家</th>
    <th style="width:7%">年份</th><th style="width:7%">格式</th><th style="width:11%">规格</th><th style="width:6%">碟数</th><th style="width:16%">来源目录</th></tr>`;
  const rows = LIST_ROWS.map((id) => {
    const a = A[id];
    const sw = a.noCover ? "background:var(--well)" : `background:linear-gradient(145deg,hsl(${a.hue} 62% 52%),hsl(${a.hue2 ?? a.hue + 30} 58% 30%))`;
    const spec = a.kind === "DSD" ? `<span class="pill">${esc(a.spec)}</span>` : esc(a.spec);
    return el(
      `list.row.${a.id}`,
      a.title,
      SRC.list,
      `<td><span class="mini" style="${sw}"></span>${esc(a.title)}</td><td class="muted">${esc(a.artist)}</td><td class="muted">${esc(a.composer ?? "")}</td><td class="num">${esc(a.year ?? "")}</td><td>${esc(a.format)}</td><td>${spec}</td><td class="num">${a.discs}</td><td class="muted mono" style="font-size:11px">${esc(a.dir)}</td>`,
      { tag: "tr", class: id === selected ? "on" : "" },
    );
  }).join("");
  return `<div class="content"><table class="table">${el("list.header", "列标题（点击排序）", SRC.list, head, { tag: "thead" })}<tbody>${rows}</tbody></table></div>`;
}

/* ---------- sheets ---------- */

function editRow(id: string, k: string, value: string, s: Source, opts: { focus?: boolean; restore?: boolean; tokens?: string[]; ph?: boolean } = {}): string {
  const inner = opts.tokens ? opts.tokens.map((t) => `<span class="tok">${esc(t)}</span>`).join("") + `<span class="ph">添加…</span>` : opts.ph ? `<span class="ph">${esc(value)}</span>` : esc(value);
  return el(
    `edit.field.${id}`,
    `${k}：${value}`,
    SRC.edit,
    `<span class="k">${esc(k)}</span><span class="input${opts.focus ? " focus" : ""}">${inner}</span>${srcBadge(`edit.field.${id}.source`, s, SRC.edit)}${el(`edit.field.${id}.restore`, "恢复自动值", SRC.edit, "恢复自动值", { tag: "span", class: `link${opts.restore ? "" : " off"}` })}`,
    { class: "form-row" },
  );
}

function editSheet(): string {
  return `<div class="scrim">${el(
    "edit.sheet",
    "编辑专辑信息",
    SRC.edit,
    `<div class="sheet-h"><h2>编辑专辑信息</h2><p>只改 Voon 的本地索引，不写回 NAS 上的文件。改过的字段标为「手动」，重新扫描不会覆盖。</p></div>
    <div class="sheet-b">
      ${editRow("title", "标题", "Beethoven: Symphony No. 5 & 7", "tag")}
      ${editRow("artist", "艺术家", "Carlos Kleiber, Wiener Philharmoniker", "tag")}
      ${editRow("composer", "作曲家", "Ludwig van Beethoven", "tag")}
      ${editRow("performer", "演奏者", "（无）", "tag", { ph: true })}
      ${editRow("conductor", "指挥", "Carlos Kleiber", "dir")}
      ${editRow("orchestra", "乐团", "Wiener Philharmoniker", "manual", { focus: true, restore: true })}
      <div class="form-row"><span></span><span class="hint">自动值：「Weiner Philharmoniker」（目录名解析，原目录名有拼写错误）</span><span></span><span></span></div>
      ${editRow("year", "年份", "1996", "dir")}
      ${editRow("genre", "流派", "古典", "tag")}
      ${editRow("tags", "标签", "", "manual", { tokens: ["交响曲", "名演"], restore: true })}
    </div>
    <div class="sheet-f">
      ${btn("edit.restore-all", "全部恢复自动值", SRC.edit, "quiet", "undo")}
      <span class="grow"></span>
      ${btn("edit.cancel", "取消", SRC.edit)}
      ${btn("edit.save", "存储", SRC.edit, "blue")}
    </div>`,
    { class: "sheet wide" },
  )}</div>`;
}

function miniAlbum(id: string, a: Album, meta: string, src: string): string {
  return el(id, a.title, src, `${coverArt(a, { size: 44 })}<div style="min-width:0"><div class="tt">${esc(a.title)}</div><div class="mm">${esc(meta)}</div></div>`, { class: "mini-al" });
}

function mergeSheet(): string {
  const k1 = A.kuraki1;
  const k2 = A.kuraki2;
  const merged: Album = { ...k1, title: "MAI KURAKI BEST 151A -LOVE & HOPE-" };
  return `<div class="scrim">${el(
    "merge.sheet",
    "合并专辑",
    SRC.merge,
    `<div class="sheet-h" style="display:flex;align-items:center;gap:12px"><div style="flex:1"><h2>合并 2 张专辑</h2><p>两个目录被识别成了两张专辑。合并后是一张专辑、两张碟，文件位置不变。</p></div>
      ${el("merge.mode", "模式：合并 / 拆分", SRC.merge, `<span class="on">${icon("merge", 13)}合并</span><span>${icon("split", 13)}拆分</span>`, { class: "seg" })}</div>
    <div class="sheet-b">
      <div class="mrg">
        <div class="col">
          ${miniAlbum("merge.source.1", k1, "15 轨 · 24/96 · Disc 1[…] 目录", SRC.merge)}
          ${miniAlbum("merge.source.2", k2, "15 轨 · 24/96 · Disc 2[…] 目录", SRC.merge)}
        </div>
        <div class="arrow">${icon("arrow", 20)}</div>
        <div class="col">
          ${el("merge.result", "合并结果预览", SRC.merge, `${coverArt(merged, { size: 44 })}<div style="min-width:0"><div class="tt">MAI KURAKI BEST 151A -LOVE &amp; HOPE-</div><div class="mm">倉木麻衣 · 2 碟 · 30 轨</div></div>`, { class: "mini-al", extra: 'style="border-color:var(--select);background:var(--select-soft)"' })}
          <div class="box" style="padding:8px 10px">
            ${el("merge.disc.1", "碟 1 ← Disc 1 目录", SRC.merge, `<span class="muted">碟 1</span>　[Hires]倉木麻衣 … Disc 1[24bit 96khz FLAC]`, { class: "files" })}
            ${el("merge.disc.2", "碟 2 ← Disc 2 目录", SRC.merge, `<span class="muted">碟 2</span>　[Hires]倉木麻衣 … Disc 2[24bit 96khz FLAC]`, { class: "files" })}
          </div>
        </div>
      </div>
      <div style="display:flex;gap:28px;margin-top:6px">
        <div>
          <div class="hint" style="margin-bottom:4px">以哪张的信息为准</div>
          ${el("merge.primary.1", "以 Disc 1 为准", SRC.merge, `<i></i>Disc 1（标题去掉「Disc 1」）`, { class: "radio on" })}
          ${el("merge.primary.2", "以 Disc 2 为准", SRC.merge, `<i></i>Disc 2`, { class: "radio" })}
        </div>
        <div>
          <div class="hint" style="margin-bottom:4px">封面</div>
          ${el("merge.cover.1", "封面用 Disc 1 的 cover.jpg", SRC.merge, `<i></i>Disc 1 的 cover.jpg`, { class: "radio on" })}
          ${el("merge.cover.2", "封面用 Disc 2 的 cover.jpg", SRC.merge, `<i></i>Disc 2 的 cover.jpg`, { class: "radio" })}
        </div>
      </div>
    </div>
    <div class="sheet-f"><span class="hint">合并会记为一条手动整理，可在「编辑信息」里撤销。</span><span class="grow"></span>${btn("merge.cancel", "取消", SRC.merge)}${btn("merge.confirm", "合并", SRC.merge, "blue")}</div>`,
    { class: "sheet wide" },
  )}</div>`;
}

function splitSheet(): string {
  const groups = [
    { id: "g1", t: "Sarah Àlainn - Anima", n: 13, by: "标签 ALBUM" },
    { id: "g2", t: "宮本笑里 - Life", n: 5, by: "标签 ALBUM" },
    { id: "g3", t: "Yoonie Han - Hollywood Romance", n: 13, by: "标签 ALBUM" },
    { id: "g4", t: "（无专辑标签）", n: 37, by: "保留在原专辑" },
  ];
  return `<div class="scrim">${el(
    "split.sheet",
    "拆分专辑",
    SRC.merge,
    `<div class="sheet-h" style="display:flex;align-items:center;gap:12px"><div style="flex:1"><h2>拆分「Hi-res Selection」</h2><p>这个目录里平铺了 500 个音频文件，按内嵌标签的 ALBUM 字段可以分出 41 张专辑。文件位置不变。</p></div>
      ${el("split.mode", "模式：合并 / 拆分", SRC.merge, `<span>${icon("merge", 13)}合并</span><span class="on">${icon("split", 13)}拆分</span>`, { class: "seg" })}</div>
    <div class="sheet-b">
      <div style="display:flex;gap:16px;align-items:center">
        <span class="hint">拆分依据</span>
        ${el("split.by.album", "按标签 ALBUM", SRC.merge, `<i></i>标签 ALBUM`, { class: "radio on" })}
        ${el("split.by.artist", "按标签 ALBUMARTIST", SRC.merge, `<i></i>标签 ALBUMARTIST`, { class: "radio" })}
        ${el("split.by.manual", "手动选择曲目", SRC.merge, `<i></i>手动选择曲目`, { class: "radio" })}
      </div>
      <div class="box">
        ${groups
          .map((g) =>
            el(`split.group.${g.id}`, g.t, SRC.merge, `<span class="chk on" style="height:auto"><span class="box2">${icon("check", 10, 3)}</span></span><span style="flex:1">${esc(g.t)}</span><span class="muted">${g.n} 轨 · ${esc(g.by)}</span>`, { class: "rule" }),
          )
          .join("")}
        <div class="rule"><span class="muted">… 另 37 组</span></div>
      </div>
    </div>
    <div class="sheet-f"><span class="hint">将新建 41 张专辑，37 个无标签文件保留在「Hi-res Selection」。</span><span class="grow"></span>${btn("split.cancel", "取消", SRC.merge)}${btn("split.confirm", "拆分为 41 张", SRC.merge, "blue")}</div>`,
    { class: "sheet wide" },
  )}</div>`;
}

function coverSheet(): string {
  const base = A.brandenburg;
  const c = [
    { id: "cover-jpg", fn: "cover.jpg", px: "1000×1000 · 目录图片", a: base, on: true },
    { id: "back-jpg", fn: "back.jpg", px: "1000×790 · 目录图片", a: { ...base, ini: "BACK", hue: 160, hue2: 190 } },
    { id: "embedded-cd1", fn: "CD1/CDImage.ape 内嵌", px: "500×500 · 内嵌封面", a: { ...base, ini: "BWV", hue: 140, hue2: 100 } },
    { id: "embedded-cd2", fn: "CD2/CDImage.ape 内嵌", px: "500×500 · 内嵌封面", a: { ...base, ini: "Vol.2", hue: 40, hue2: 20 } },
  ];
  return `<div class="scrim">${el(
    "cover.sheet",
    "选择封面",
    SRC.cover,
    `<div class="sheet-h"><h2>选择封面</h2><p>候选来自专辑目录及碟子目录里的图片和音频内嵌封面（Booklet/、Scans/ 按忽略规则不参与）。自动选择优先 cover / folder / front 命名、近正方形、分辨率最高的。</p></div>
    <div class="sheet-b"><div class="cands">
      ${c
        .map((x) =>
          el(`cover.candidate.${x.id}`, x.fn, SRC.cover, `${coverArt(x.a as Album)}<div class="fn">${esc(x.fn)}</div><div class="px">${esc(x.px)}${x.on ? " · 当前" : ""}</div>`, { class: `cand${x.on ? " on" : ""}` }),
        )
        .join("")}
    </div>
    <div class="hint">${icon("eye", 12)} 不会复制或写入图片文件，只在本地索引里记住选择。</div></div>
    <div class="sheet-f">${btn("cover.from-file", "从其他文件选择…", SRC.cover, "quiet", "image")}<span class="grow"></span>${btn("cover.cancel", "取消", SRC.cover)}${btn("cover.confirm", "使用这张", SRC.cover, "blue")}</div>`,
    { class: "sheet wide" },
  )}</div>`;
}

/* ---------- views ---------- */

function welcome(theme: ThemeName): string {
  return shell(
    theme,
    `${sidebar("none", { empty: true })}<section class="main">${toolbar("Voon", { rescan: false })}<div class="content">
      <div class="empty">
        <div class="glyph">${icon("music", 40, 1.6)}</div>
        <h2>添加你的音乐库</h2>
        <p>选择 NAS 上存放音乐的文件夹。Voon 会读取标签、CUE 和目录名，建立一份本地索引；NAS 不在线时也能浏览。</p>
        ${el(
          "onboarding.drop",
          "把文件夹拖到这里，或点选择文件夹",
          SRC.onboarding,
          `${el("onboarding.path", ROOT, SRC.onboarding, `${icon("nas", 16)}<span class="mono grow">${ROOT}</span><span class="muted">SMB · NAS1</span>`, { class: "path-row" })}
           <div style="display:flex;gap:8px;justify-content:center">${btn("onboarding.choose", "选择文件夹…", SRC.onboarding, "lg", "folder")}${btn("onboarding.start", "开始扫描", SRC.onboarding, "primary lg")}</div>`,
          { class: "drop" },
        )}
        ${el("onboarding.readonly-note", "只读：不会修改 NAS 上的任何文件", SRC.onboarding, `${icon("eye", 13)}<span>只读：不会修改、移动或删除 NAS 上的任何文件</span>`, { class: "note" })}
        ${el("onboarding.more-roots", "之后可在设置里添加更多根目录", SRC.onboarding, `<span>之后可以在「设置 › 音乐库」里添加更多根目录</span>`, { class: "note" })}
      </div></div></section>`,
  );
}

function firstScan(theme: ThemeName): string {
  const ids = ["evanescence", "kv563", "aga", "suwanai", "kleiber", "brandenburg", "brahms", "grumiaux", "bachdsd", "emiri", "tongli"];
  const banner = `<div class="banner scan">${icon("refresh", 15)}<div class="grow">
      ${el("scan.headline", "正在扫描 /Volumes/NAS1/Music", SRC.scan, `<b>首次扫描 · 已发现 214 张专辑</b>　<span class="muted">2,031 / 5,658 个文件 · 约 6 分钟</span>`)}
      ${el("scan.progress", "扫描进度 36%", SRC.scan, `<i style="width:36%"></i>`, { class: "bar" })}
      ${el("scan.current", "当前：Philips Duo/[438323-2]Mozart Complete Violin Concertos [Graumiaux]/CD2", SRC.scan, `正在读取 Philips Duo/[438323-2]Mozart Complete Violin Concertos [Graumiaux]/CD2/…cue`, { class: "muted", extra: 'style="font-size:11.5px;margin-top:3px"' })}
    </div>${btn("scan.cancel", "取消", SRC.scan)}</div>`;
  return app(theme, "library", `${toolbar("音乐库", { sub: "扫描中，结果会陆续出现", view: "grid" })}${banner}<div class="content">${grid(ids, undefined, { ghosts: 7, newFrom: 9 })}</div>`, "", { scanning: true });
}

function mainGrid(theme: ThemeName, opts: { overlay?: string; selected?: string; detail?: string; offline?: boolean } = {}): string {
  const sel = opts.selected ?? "suwanai";
  const banner = opts.offline
    ? `<div class="banner warn">${icon("unplug", 15)}<div class="grow">${el("offline.banner", "NAS1 未挂载", SRC.offline, `<b style="color:inherit">NAS1 未挂载</b>　正在使用本地索引（上次扫描 ${STATS.lastScan}）。浏览、搜索、编辑信息照常可用；打开文件和重新扫描需要 NAS 在线。`)}</div>${btn("offline.reconnect", "重新连接", SRC.offline)}</div>`
    : "";
  return app(
    theme,
    "library",
    `${toolbar("音乐库", { sub: `${STATS.albums} 张专辑`, view: "grid" })}${banner}<div class="content">${grid(GRID_MAIN, sel)}</div>`,
    opts.detail ?? suwanaiDetail({ offline: opts.offline }),
    { offline: opts.offline },
    opts.overlay ?? "",
  );
}

function mainList(theme: ThemeName): string {
  return app(theme, "library", `${toolbar("音乐库", { sub: `${STATS.albums} 张专辑`, view: "list" })}${listView("brandenburg")}`, brandenburgDetail());
}

function collectionView(theme: ThemeName): string {
  const ids = ["brandenburg", "brahms", "grumiaux"];
  const extra: Album[] = [
    { ...A.brahms, id: "pd-438326", title: "Schubert: Piano Sonatas", artist: "Alfred Brendel", ini: "D.960", hue: 280, hue2: 250, catalog: "438 326-2" },
    { ...A.brahms, id: "pd-438329", title: "Vivaldi: L'estro armonico", artist: "I Musici", ini: "Op.3", hue: 20, hue2: 50, catalog: "438 329-2" },
    { ...A.brahms, id: "pd-438332", title: "Haydn: Symphonies Nos. 94, 100, 101", artist: "Colin Davis", ini: "Hob.I", hue: 190, hue2: 220, catalog: "438 332-2" },
    { ...A.brahms, id: "pd-438335", title: "Mendelssohn: Symphonies 3 & 4", artist: "Kurt Masur", ini: "Op.90", hue: 60, hue2: 90, catalog: "438 335-2" },
    { ...A.brahms, id: "pd-438338", title: "Dvořák: Symphonies 7–9", artist: "Colin Davis", ini: "Op.95", hue: 340, hue2: 10, catalog: "438 338-2" },
    { ...A.brahms, id: "pd-438341", title: "Liszt: Piano Concertos", artist: "Sviatoslav Richter", ini: "S.124", hue: 230, hue2: 260, catalog: "438 341-2" },
    { ...A.brahms, id: "pd-438344", title: "Chopin: Nocturnes", artist: "Claudio Arrau", ini: "Op.27", hue: 110, hue2: 80, catalog: "438 344-2" },
    { ...A.brahms, id: "pd-438347", title: "Mozart: Requiem · Masses", artist: "Colin Davis", ini: "K.626", hue: 0, hue2: 330, catalog: "438 347-2" },
    { ...A.brahms, id: "pd-438350", title: "Debussy · Ravel: Orchestral Works", artist: "Bernard Haitink", ini: "L.86", hue: 170, hue2: 140, catalog: "438 350-2" },
  ];
  extra.forEach((a) => (A[a.id] = a));
  const all = [...ids, ...extra.map((a) => a.id)];
  const header = `<div class="banner" style="gap:16px;padding:14px 18px;background:var(--well)">
    ${el("collection.cover", "套装：Philips Duo", SRC.collection, `<div style="display:flex">${["brandenburg", "brahms", "grumiaux"].map((id, i) => `<div style="margin-left:${i ? -26 : 0}px">${coverArt(A[id], { size: 54 })}</div>`).join("")}</div>`)}
    <div class="grow">
      ${el("collection.title", "Philips Duo", SRC.collection, `<div style="font-size:16px;font-weight:600;line-height:22px">Philips Duo</div>`)}
      ${el("collection.meta", "193 张专辑 · 386 碟 · 118 GB · 套装由外层目录识别", SRC.collection, `<span class="muted">193 张专辑 · 386 碟 · 118 GB · 外层目录只包含子专辑，识别为套装</span>`)}
    </div>
    ${el("collection.sort", "排序：唱片编号", SRC.collection, `${icon("sort", 14)}<span>唱片编号</span>`, { class: "btn quiet", tag: "span" })}
  </div>`;
  return app(theme, "collections", `${toolbar("套装", { sub: "Philips Duo", view: "grid" })}${header}<div class="content">${grid(all, "brandenburg", { idPrefix: "collection" })}</div>`, brandenburgDetail());
}

function composerView(theme: ThemeName): string {
  const list = COMPOSERS.map((c) =>
    el(
      `composers.item.${c.id}`,
      `${c.name}（${c.n} 张）`,
      SRC.composers,
      `<span class="avatar" style="background:linear-gradient(145deg,hsl(${c.hue} 55% 55%),hsl(${c.hue + 30} 55% 35%))">${esc(c.cn.slice(0, 1))}</span><div><div class="nm">${esc(c.name)}</div><div class="cn">${esc(c.cn)}</div></div><span class="n">${c.n}</span>`,
      { class: `comp${c.id === "mozart" ? " on" : ""}` },
    ),
  ).join("");
  const group = (id: string, label: string, ids: string[]) =>
    `${el(`composers.group.${id}`, label, SRC.composers, `${esc(label)}<span class="muted" style="font-weight:400">${ids.length} 张</span>`, { class: "grp-h" })}${grid(ids, ids[0] === "kv563" ? "kv563" : undefined, { idPrefix: `composers.${id}`, cols: 5 })}`;
  return app(
    theme,
    "composers",
    `${toolbar("作曲家", { sub: "112 位 · 按专辑数", view: null })}<div class="content"><div class="split">
      <div class="panel" style="width:260px;gap:2px">${el("composers.filter", "筛选作曲家", SRC.composers, `${icon("search", 13)}<span>筛选作曲家</span>`, { class: "search", extra: 'style="width:100%;margin-bottom:8px"' })}<div class="alpha">${list}</div></div>
      <div style="flex:1;min-width:0;overflow:hidden">
        <div class="grp-h" style="font-size:16px;color:var(--ink);padding-top:16px">W.A. Mozart · 莫扎特 <span class="muted" style="font-weight:400;font-size:12px">214 张专辑，其中 180 张来自「莫扎特全集【180CD】」</span></div>
        ${group("chamber", "钢琴与室内乐", ["kv563", "langlang", "vikingur"])}
        ${group("concerto", "协奏曲", ["mozartvn", "grumiaux"])}
      </div>
    </div></div>`,
    "",
  );
}

function check(id: string, label: string, n: number | string, on = false): string {
  return el(`facets.${id}`, `${label}（${n}）`, SRC.facets, `<span class="box2">${on ? icon("check", 10, 3) : ""}</span><span>${esc(label)}</span><span class="n">${n}</span>`, { class: `chk${on ? " on" : ""}` });
}

function facetView(theme: ThemeName): string {
  const panel = `<div class="panel">
    <h5>类型</h5>${check("kind.pcm", "PCM", 541)}${check("kind.dsd", "DSD", 45, true)}
    <h5>格式</h5>${check("format.flac", "FLAC", 389)}${check("format.ape", "APE", 142)}${check("format.dsf", "DSF", 41, true)}${check("format.dff", "DFF", 4, true)}${check("format.wv", "WV", 3)}${check("format.wav", "WAV", 7)}
    <h5>规格</h5>${check("spec.cd", "CD 16/44.1", 311)}${check("spec.hr96", "Hi-Res 24/96", 148)}${check("spec.hr192", "24/192", 22)}${check("spec.dxd", "DXD 352.8k", 3)}${check("spec.dsd64", "DSD64", 12)}${check("spec.dsd128", "DSD128", 6)}${check("spec.dsd256", "DSD256", 27)}
    <h5>语种</h5>${check("lang.inst", "器乐", 402)}${check("lang.zh", "国语", 46)}${check("lang.yue", "粤语", 29)}${check("lang.ja", "日语", 38)}${check("lang.en", "英语", 61)}
    <h5>年代</h5>${check("era.1990", "1990 年代", 214)}${check("era.2000", "2000 年代", 88)}${check("era.2010", "2010 年代", 171)}${check("era.2020", "2020 年代", 63)}
  </div>`;
  const chips = `<div class="chips">
    ${el("facets.active.dsd", "已选：类型 DSD", SRC.facets, `类型：DSD ${icon("x", 11, 2.2)}`, { tag: "span", class: "chipf" })}
    ${el("facets.active.format", "已选：格式 DSF、DFF", SRC.facets, `格式：DSF、DFF ${icon("x", 11, 2.2)}`, { tag: "span", class: "chipf" })}
    <span class="muted" style="font-size:12px">45 张专辑</span>
    ${el("facets.clear", "清除筛选", SRC.facets, "清除", { tag: "span", class: "link" })}
    <span style="flex:1"></span>
    ${btn("facets.save-smart", "存为合集", SRC.facets, "quiet", "plus")}
  </div>`;
  return app(theme, "facets", `${toolbar("分面", { view: "grid" })}<div class="content"><div class="split">${panel}<div style="flex:1;min-width:0">${chips}${grid(["bachdsd", "quiet", "guzheng", "mozartvn", "minnaar"], "quiet", { idPrefix: "facets.result", cols: 5 })}</div></div></div>`, "");
}

function tagsView(theme: ThemeName): string {
  const tags = [
    { id: "night", t: "睡前小提琴", c: "#b4532a", n: 24 },
    { id: "solo", t: "独奏", c: "#2f6fdc", n: 41 },
    { id: "symphony", t: "交响曲", c: "#7a3fb0", n: 37 },
    { id: "great", t: "名演", c: "#c08a00", n: 19 },
    { id: "test", t: "试音", c: "#2e8540", n: 17 },
    { id: "yue", t: "粤语", c: "#c0352b", n: 29 },
    { id: "ost", t: "原声", c: "#0f8a8a", n: 11 },
    { id: "todo-check", t: "待核对", c: "#6e6e73", n: 6 },
  ];
  const stacks = [
    { id: "night", t: "睡前小提琴", n: 24, d: "手动合集 · 3 天前更新" },
    { id: "dsd", t: "DSD 试音碟", n: 17, d: "手动合集 · 11 月 2 日" },
    { id: "car", t: "车载 · 粤语", n: 31, d: "手动合集 · 10 月 28 日" },
    { id: "kempff", t: "Kempff 贝多芬奏鸣曲全集", n: 5, d: "手动合集 · 有序" },
    { id: "gift", t: "送爸妈的", n: 12, d: "手动合集" },
  ];
  const tagRows = tags
    .map((t) => el(`tags.tag.${t.id}`, `标签：${t.t}（${t.n}）`, SRC.tags, `<span style="width:10px;height:10px;border-radius:50%;background:${t.c};flex:none"></span><span style="flex:1">${esc(t.t)}</span><span class="muted">${t.n} 张</span>${iconBtn(`tags.tag.${t.id}.more`, "重命名、改颜色、删除", SRC.tags, "dots")}`, { class: "rule", extra: t.id === "night" ? 'style="background:var(--select-soft)"' : "" }))
    .join("");
  const stackRows = stacks
    .map((s) => el(`tags.stack.${s.id}`, `合集：${s.t}`, SRC.tags, `${icon("stack", 15)}<span style="flex:1">${esc(s.t)}<br><span class="muted" style="font-size:11px">${esc(s.d)}</span></span><span class="muted">${s.n} 张</span>`, { class: "rule", extra: 'style="padding:8px 12px"' }))
    .join("");
  return app(
    theme,
    "tags",
    `${toolbar("标签与合集", { view: null })}<div class="content" style="display:grid;grid-template-columns:1fr 1fr;gap:18px;padding:18px;align-content:start">
      <div><div style="display:flex;align-items:center;margin-bottom:8px"><b style="flex:1">标签</b>${btn("tags.new-tag", "新建标签", SRC.tags, "quiet", "plus")}</div>
        <div class="box" style="background:var(--win)">${tagRows}</div>
        <p class="hint" style="margin-top:8px">标签是给专辑贴的自由关键词，一张专辑可以有多个。只存在本地索引，不写回文件。</p></div>
      <div><div style="display:flex;align-items:center;margin-bottom:8px"><b style="flex:1">合集</b>${btn("tags.new-stack", "新建合集", SRC.tags, "quiet", "plus")}</div>
        <div class="box" style="background:var(--win)">${stackRows}</div>
        <p class="hint" style="margin-top:8px">合集是手动挑选、可排序的专辑列表。第一期不做智能合集；分面里的「存为合集」会把当前结果固定成一份手动合集。</p>
        <div style="margin-top:14px"><b style="font-size:12px">合集「睡前小提琴」预览</b>
        ${grid(["suwanai", "emiri", "bachdsd", "mozartvn"], undefined, { idPrefix: "tags.preview", cols: 4 }).replace('class="grid"', 'class="grid" style="padding:10px 0;grid-template-columns:repeat(4,minmax(0,1fr))"')}</div>
      </div>
    </div>`,
    "",
  );
}

type Issue = { id: string; a: Album; reason: string; kind: string; detail: string; acts: [string, string, string?][]; level?: "warn" | "danger" };

function inboxView(theme: ThemeName, overlay = ""): string {
  const issues: Issue[] = [
    { id: "numeric", a: A.numeric, kind: "无标签", reason: "文件名全是数字，且没有内嵌标签", detail: "0015830558.flac 等 14 个文件 · 目录名「Unknown_Artist」也没有可用信息", acts: [["edit", "手动填写", "blue"], ["open", "在访达中显示"]], level: "danger" },
    { id: "group2007", a: A.group2007, kind: "CUE 乱码", reason: "CUE 编码无法识别，曲目名显示为乱码", detail: "猜测为 GBK（置信度 62%）· 预览：「01 姊奔捕的女孩」→「01 寂寞的女孩」", acts: [["encoding", "选择编码…", "blue"], ["skip", "忽略"]], level: "danger" },
    { id: "kuraki", a: A.kuraki1, kind: "多碟未合并", reason: "疑似同一张专辑的两张碟被识别成两张专辑", detail: "「… Disc 1[24bit 96khz FLAC]」与「… Disc 2[…]」目录名只差碟号", acts: [["merge", "合并为 2 碟", "blue"], ["skip", "不是同一张"]], level: "warn" },
    { id: "hires", a: A.hires, kind: "无封面", reason: "没有找到封面图片，音频也没有内嵌封面", detail: "目录里 500 个 FLAC 平铺，可能是多张专辑混在一起", acts: [["split", "拆分…", "blue"], ["cover", "选择封面…"]], level: "warn" },
    { id: "sens", a: A.sens, kind: "CUE 不匹配", reason: "CUE 引用的音频文件不存在", detail: "CUE 指向「SENS.wav」，目录里只有「SENS.ape」· 长度与 CUE 最后一轨的起点吻合", acts: [["relink", "改用 SENS.ape", "blue"], ["open", "在访达中显示"]], level: "warn" },
  ];
  const tabs = `<div class="tabs">
    ${el("inbox.filter.all", "全部 23", SRC.inbox, "全部 <b>23</b>", { tag: "span", class: "on" })}
    ${el("inbox.filter.untagged", "无标签 4", SRC.inbox, "无标签 4", { tag: "span" })}
    ${el("inbox.filter.cue-encoding", "CUE 乱码 3", SRC.inbox, "CUE 乱码 3", { tag: "span" })}
    ${el("inbox.filter.unmerged", "多碟未合并 5", SRC.inbox, "多碟未合并 5", { tag: "span" })}
    ${el("inbox.filter.no-cover", "无封面 9", SRC.inbox, "无封面 9", { tag: "span" })}
    ${el("inbox.filter.cue-mismatch", "CUE 不匹配 2", SRC.inbox, "CUE 不匹配 2", { tag: "span" })}
  </div>`;
  const rows = issues
    .map((i) =>
      el(
        `inbox.item.${i.id}`,
        `${i.a.dir} — ${i.reason}`,
        SRC.inbox,
        `${coverArt(i.a)}<div style="min-width:0"><div class="ttl">${esc(i.a.dir)}</div><div class="why"><span class="pill ${i.level ?? ""}">${icon("warn", 11)}${esc(i.kind)}</span>　${esc(i.reason)}<br><span style="color:var(--tertiary)">${esc(i.detail)}</span></div></div>
         <div class="acts">${i.acts.map(([k, l, kind]) => btn(`inbox.item.${i.id}.${k}`, l, SRC.inbox, kind ?? "")).join("")}</div>`,
        { class: `issue${i.id === "group2007" ? " on" : ""}` },
      ),
    )
    .join("");
  return app(theme, "inbox", `${toolbar("待处理", { sub: "23 项 · 修好后自动移出", view: null })}${tabs}<div class="content" style="margin-top:10px;border-top:0.5px solid var(--line)">${rows}</div>`, "", {}, overlay);
}

function settingsView(theme: ThemeName): string {
  const tab = (id: string, label: string, glyph: string, on = false) => el(`settings.tab.${id}`, label, SRC.settings, `${icon(glyph, 20)}<span>${label}</span>`, { class: `prefs-tab${on ? " on" : ""}` });
  const roots = [
    { id: "nas1", p: "/Volumes/NAS1/Music", s: `${STATS.albums} 张 · ${STATS.size} · ${STATS.lastScan}`, ok: true },
    { id: "local", p: "~/Music/待整理", s: "38 张 · 21.4 GB · 今天 09:02", ok: true },
  ];
  const ignore = [
    { id: "eadir", g: "@eaDir/", d: "群晖缩略图目录", on: true },
    { id: "recycle", g: "#recycle/", d: "NAS 回收站", on: true },
    { id: "dotfiles", g: ".* · ._*", d: "隐藏文件与 macOS 资源分叉（.DS_Store 等）", on: true },
    { id: "video", g: "*.m4v · *.mp4 · *.mkv", d: "视频（例如 Latin Music Video Collection）", on: true },
    { id: "docs", g: "*.log · *.pdf · *.txt · *.url", d: "抓轨日志、小册子、说明与广告链接", on: true },
    { id: "booklet", g: "Booklet/ · Scans/ · scan/", d: "小册子扫描目录（不参与封面候选）", on: true },
  ];
  return shell(
    theme,
    `<div style="height:52px;display:flex;align-items:center;justify-content:center;position:relative;background:var(--toolbar)"><div style="position:absolute;left:10px;top:0">${lights()}</div><b style="font-size:13px">音乐库</b></div>
    <div class="prefs-tabs">${tab("library", "音乐库", "library", true)}${tab("scan", "扫描", "refresh")}${tab("ignore", "忽略规则", "eye")}${tab("general", "通用", "gear")}</div>
    <div class="prefs-body">
      <div><h3>音乐库根目录</h3>
        <div class="box" style="background:var(--win)">
          ${roots.map((r) => el(`settings.root.${r.id}`, r.p, SRC.settings, `${icon(r.id === "nas1" ? "nas" : "folder", 16)}<span><span class="mono">${esc(r.p)}</span><br><span class="muted" style="font-size:11px">${esc(r.s)}</span></span><span class="pill" style="color:var(--ok)">已连接</span>${iconBtn(`settings.root.${r.id}.more`, "重新扫描此目录、在访达中显示、移除", SRC.settings, "dots")}`, { class: "rootrow" })).join("")}
        </div>
        <div style="display:flex;gap:8px;margin-top:8px">${btn("settings.add-root", "添加根目录…", SRC.settings, "", "plus")}<span style="flex:1"></span>${btn("settings.rescan", "增量重新扫描", SRC.settings, "", "refresh")}${btn("settings.full-rescan", "完整重建索引", SRC.settings, "quiet")}</div>
        <p class="hint" style="margin-top:6px">增量扫描只重读修改时间或大小变了的文件。完整重建会丢弃自动识别结果，但保留你的手动修改、标签和合集。</p>
      </div>
      <div><h3>忽略规则</h3>
        <div class="box" style="background:var(--win)">
          ${ignore.map((r) => el(`settings.ignore.${r.id}`, `${r.g} — ${r.d}`, SRC.settings, `<span class="mono">${esc(r.g)}</span><span class="muted">${esc(r.d)}</span><span class="toggle${r.on ? " on" : ""}"></span>`, { class: "rule" })).join("")}
        </div>
        <div style="display:flex;gap:8px;margin-top:8px">${btn("settings.add-rule", "添加规则…", SRC.settings, "quiet", "plus")}</div>
      </div>
      ${el("settings.index-location", "索引位置", SRC.settings, `<span class="muted">本地索引</span><span class="mono" style="font-size:11.5px">~/Library/Application Support/Voon/library.sqlite</span><span class="muted">· 38 MB</span>`, { class: "kv" })}
    </div>`,
    720,
    800,
    "small",
  );
}

/* ---------- registry ---------- */

type Partial = Omit<FrameSpec, "width"> & { width?: number };

function defineFrames(): Partial[] {
  return [
    { id: "1-welcome", number: "1", section: "开始", title: "首次启动 · 添加音乐库根目录", caption: "空状态。侧栏置灰；选好 /Volumes/NAS1/Music 后点「开始扫描」。首屏就说明只读、不动 NAS 文件。", render: welcome },
    { id: "2-first-scan", number: "2", section: "开始", title: "首次扫描进行中", caption: "后台扫描，扫出一张就显示一张（新卡片有淡入）。顶部横幅显示专辑数、文件进度、当前目录和「取消」。取消后已识别的结果保留。", assumption: "「约 6 分钟」按 SMB 千兆、首扫读标签头估算，仅示意。", render: firstScan },
    { id: "3-main-grid", number: "3", section: "主界面", title: "主界面 · 网格视图（默认）", caption: "三栏：左侧导航，中间专辑网格，右侧详情。封面角标显示规格（Hi-Res 橙、DSD 紫、CD 只写格式）。详情里每个字段后面是来源标记：标签 / CUE / 目录名 / 手动。", themeable: true, render: (t) => mainGrid(t) },
    { id: "4-main-list", number: "4", section: "主界面", title: "主界面 · 列表视图", caption: "列：专辑、艺术家/演奏者、作曲家、年份、格式、规格、碟数、来源目录。点击列标题排序；选中行在右侧显示详情（这里是 Philips Duo 的多碟专辑）。", themeable: true, render: mainList },
    { id: "5-detail-multidisc", number: "5", section: "专辑详情", title: "专辑详情 · 多碟套装（CUE 虚拟音轨）", caption: "Philips Duo/[438317-2]：CD1、CD2 子目录合并成一张专辑两张碟；每张碟是 CDImage.ape + .cue，按 CUE 显示虚拟音轨，原文件不拆。Booklet 目录忽略，封面取 cover.jpg。", themeable: true, render: (t) => mainGrid(t, { selected: "brandenburg", detail: brandenburgDetail() }) },
    { id: "6-detail-classical", number: "6", section: "专辑详情", title: "专辑详情 · 古典字段", caption: "作曲家、作品、指挥、乐团、演奏者单列为正式字段。乐团被手动改正过（目录名原文是 Weiner），所以来源标为「手动」。", render: (t) => mainGrid(t, { selected: "kleiber", detail: kleiberDetail() }) },
    { id: "7-edit-sheet", number: "7", section: "整理操作", title: "编辑专辑信息", caption: "专辑级字段（第一期不做曲目级）。每行右侧：当前来源 + 「恢复自动值」。正在编辑的乐团显示自动值供对照。只改本地索引。", render: (t) => mainGrid(t, { selected: "kleiber", detail: kleiberDetail(), overlay: editSheet() }) },
    { id: "8-merge", number: "8", section: "整理操作", title: "合并专辑", caption: "从「待处理」的「合并为 2 碟」进入（也可在网格多选后右键）。倉木麻衣 BEST 151A 的 Disc 1 / Disc 2 两个目录 → 一张专辑两张碟。选主信息来源和封面，预览碟序。", render: (t) => inboxView(t, mergeSheet()) },
    { id: "8b-split", number: "8b", section: "整理操作", title: "拆分专辑", caption: "同一个 sheet 的拆分模式：Hi-res Selection 目录平铺了 500 个文件，按标签 ALBUM 分组拆成多张专辑。", assumption: "41 组、37 个无标签文件是示意数字，真实值要扫描后才知道。", render: (t) => inboxView(t, splitSheet()) },
    { id: "9-cover-picker", number: "9", section: "整理操作", title: "选择封面", caption: "候选：专辑目录图片（cover.jpg、back.jpg）和音频内嵌封面。自动选择规则写在说明里，手动选择只记在索引。", assumption: "这张专辑的 APE 是否带内嵌封面，扫描报告看不出来，两张内嵌候选是示意。", render: (t) => mainGrid(t, { selected: "brandenburg", detail: brandenburgDetail(), overlay: coverSheet() }) },
    { id: "10-collection", number: "10", section: "浏览", title: "套装 · Philips Duo", caption: "只包含子专辑的外层目录识别为「套装」。顶部是套装摘要（193 张 · 386 碟），下面是成员专辑，可按唱片编号排序。", assumption: "除前三张外，其余 Philips Duo 成员是示意数据，扫描报告只展开到前几张。", render: collectionView },
    { id: "11-composers", number: "11", section: "浏览", title: "作曲家", caption: "左侧作曲家按专辑数排序，右侧按体裁分组。「作品」级归并（同曲不同录音）放到第二期。", render: composerView },
    { id: "12-facets", number: "12", section: "浏览", title: "分面筛选", caption: "格式、规格、DSD/PCM、语种、年代，计数随筛选联动。已选条件以标签形式显示在结果上方；「存为合集」把当前结果固定成手动合集。", assumption: "各分面计数是按扫描报告的文件数估算到专辑级的示意值。", render: facetView },
    { id: "13-tags", number: "13", section: "整理", title: "标签与合集管理", caption: "标签：自由关键词，多选。合集：手动挑选、可排序。都只存本地索引。", render: tagsView },
    { id: "14-inbox", number: "14", section: "整理", title: "待处理", caption: "自动识别不了或有冲突的专辑集中在这里，每条写明原因并给出修复动作：手动填写、选择 CUE 编码、合并、拆分、选择封面、改用实际音频文件。修好后自动移出。", themeable: true, render: (t) => inboxView(t) },
    { id: "15-offline", number: "15", section: "状态", title: "NAS 未挂载（离线）", caption: "本地 SQLite 索引照常浏览、搜索、编辑；横幅说明状态并提供「重新连接」。需要文件的操作（用默认播放器打开、重新扫描）置灰。", render: (t) => mainGrid(t, { offline: true }) },
    { id: "16-settings", number: "16", section: "状态", title: "设置 · 音乐库", caption: "根目录列表（可多个）、增量重新扫描 / 完整重建、忽略规则（视频、日志、小册子、隐藏文件默认忽略）、索引文件位置。", render: settingsView },
  ];
}

export function allFrames(): FrameSpec[] {
  const base = defineFrames().map((f) => ({ ...f, width: f.width ?? (f.id === "16-settings" ? 792 : W + 72) }));
  const dark = base
    .filter((f) => f.themeable)
    .map((f) => ({ ...f, id: `${f.id}-dark`, number: `${f.number}d`, section: "深色模式", title: `${f.title}（深色）`, themeable: false }));
  return [...base, ...dark];
}
