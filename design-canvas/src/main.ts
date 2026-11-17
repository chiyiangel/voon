import "./product.css";
import "./canvas.css";
import { beginFrame } from "./catalog";
import { mountCanvas } from "./canvas";
import { allFrames } from "./frames";
import { esc } from "./catalog";

const world = document.querySelector<HTMLElement>("#world")!;
const icon = (path: string) =>
  `<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${path}</svg>`;

let rowEl: HTMLElement | null = null;
let sectionName = "";
const frames = allFrames();
for (const frame of frames) {
  if (frame.section !== sectionName) {
    sectionName = frame.section;
    const sectionEl = document.createElement("section");
    sectionEl.className = "c-section";
    sectionEl.innerHTML = `<h2>${esc(frame.section)}</h2>`;
    rowEl = document.createElement("div");
    rowEl.className = "c-row";
    sectionEl.append(rowEl);
    world.append(sectionEl);
  }
  beginFrame(frame.id);
  const theme = frame.id.endsWith("-dark") ? "dark" : "light";
  const stage = frame.render(theme);
  const article = document.createElement("article");
  article.className = "c-frame";
  article.dataset.frame = frame.id;
  article.style.width = `${frame.width}px`;
  article.innerHTML = `
    <header class="c-cap">
      <h3>${esc(`${frame.number} · ${frame.title}`)}</h3>
      <div class="c-actions">
        <button type="button" data-act="expand" title="放大这一帧">${icon('<path d="M9 4H4v5M15 4h5v5M20 15v5h-5M4 15v5h5"/>')}</button>
        <button type="button" data-act="download" title="下载这一帧">${icon('<path d="M12 4v11m-4-4 4 4 4-4M5 20h14"/>')}</button>
      </div>
    </header>
    <p class="c-note">${esc(frame.caption)}</p>
    ${frame.assumption ? `<p class="c-assume">假设：${esc(frame.assumption)}</p>` : ""}
    <div class="c-stage">${stage}</div>`;
  rowEl?.append(article);
}

mountCanvas(frames.map(({ id, number, title, section }) => ({ id, number, title, section })));
