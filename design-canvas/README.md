# Voon 设计画布

A static, pannable canvas with every planned Voon screen side by side, for reviewing layout before any GPUI UI is written. Frames are hand-built HTML mock-ups, not screenshots, and the mock library uses folder names from a real scan of `/Volumes/NAS1/Music`. Cover art is generated (gradients + initials).

## Run

```bash
cd design-canvas
npm install
npm run dev      # local
npm run build    # dist/index.html (single file, JS+CSS inlined) + dist/element-index.json
```

Vercel: Root Directory `design-canvas`, build `npm run build`, output `dist` (see `vercel.json`).

## Moving around

- Scroll pans; hold Space or switch to 平移 (H) and drag. ⌘/Ctrl + scroll or pinch zooms.
- 选择 (V) is the default: hover outlines a control, click copies its reference.
- 适应 / `0` fits everything, 100% / `1` returns to actual size. The 跳转 menu and a frame's expand button zoom to one frame. The URL hash keeps the frame and camera (`#f=5-detail-multidisc`).

## Element references

A click copies one line, for pasting into a review message:

```text
[voon-design] frame=5-detail-multidisc > el=detail.disc.1 | label="CD1 · CD1/CDImage.ape + CDImage.cue" | src=crates/voon-app/src/ui/album_detail.rs
```

| Field | Meaning |
| --- | --- |
| `frame` | Stable frame id (listed in `element-index.json`). Dark twins end in `-dark`. |
| `el` | Stable ascii id, unique inside the frame; the same control keeps the same id across frames. |
| `label` | Visible copy. |
| `src` | The planned GPUI module under `crates/voon-app/src/ui/` that will draw it (not written yet). |

## Frames

| # | Frame |
| --- | --- |
| 1 | 首次启动 · 添加音乐库根目录 |
| 2 | 首次扫描进行中 |
| 3 | 主界面 · 网格视图 |
| 4 | 主界面 · 列表视图 |
| 5 | 专辑详情 · 多碟套装（CUE 虚拟音轨） |
| 6 | 专辑详情 · 古典字段 |
| 7 | 编辑专辑信息 |
| 8 / 8b | 合并 / 拆分专辑 |
| 9 | 选择封面 |
| 10 | 套装 · Philips Duo |
| 11 | 作曲家 |
| 12 | 分面筛选 |
| 13 | 标签与合集 |
| 14 | 待处理 |
| 15 | NAS 未挂载（离线） |
| 16 | 设置 · 音乐库 |
| 3d, 4d, 5d, 14d | 深色模式 |

Frames with `themeable: true` in `src/frames.ts` get a dark twin automatically. Amber 「假设」 notes mark numbers or details that are illustrative rather than taken from the scan.

The canvas mechanism (camera, click-to-copy, element index) follows `chiyiangel/visper`'s `design-canvas/`.
