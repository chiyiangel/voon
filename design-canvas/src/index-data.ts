import { beginFrame, catalog, resetCatalog } from "./catalog";
import { allFrames } from "./frames";

export function emitIndex(): string {
  resetCatalog();
  const frames = allFrames();
  for (const frame of frames) {
    beginFrame(frame.id);
    frame.render(frame.id.endsWith("-dark") ? "dark" : "light");
  }
  const elements = catalog();
  return JSON.stringify(
    {
      copyFormat: '[voon-design] frame=<frame> > el=<id> | label="<label>" | src=<planned module>',
      idConvention:
        "ids are stable ascii paths unique inside a frame; the same control keeps the same id across frames. label is the visible copy. src is the planned GPUI module under crates/voon-app/src/ui (not written yet).",
      frameCount: frames.length,
      elementCount: elements.length,
      frames: frames.map((f) => ({ id: f.id, number: f.number, title: f.title, section: f.section, assumption: f.assumption ?? null })),
      elements,
    },
    null,
    2,
  );
}
