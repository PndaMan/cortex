// Scheme allow-listing for LLM-/web-derived URLs, so a javascript:/data:text/html
// value can't execute when rendered into an <a href> or <img src>.

/** Safe href for a link: http(s)/mailto/asset/tauri or app-relative; else "" (blocked). */
export function safeUrl(u: string | undefined | null): string {
  const s = (u ?? "").trim();
  if (!s) return "";
  if (/^(https?:|mailto:|asset:|tauri:|[/#])/i.test(s)) return s;
  if (s.startsWith("//")) return "https:" + s; // protocol-relative → https
  return ""; // javascript:, data:, vbscript:, file:, unknown → block
}

/** Safe src for an image: http(s)/asset/blob/tauri/app-relative, or data:image only. */
export function safeImgSrc(u: string | undefined | null): string {
  const s = (u ?? "").trim();
  if (!s) return "";
  if (/^(https?:|asset:|blob:|tauri:|\/)/i.test(s)) return s;
  if (/^data:image\//i.test(s)) return s; // inline image data ok; data:text/html blocked
  return "";
}
