import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { parse } from "svelte/compiler";
import { translateText } from "../src/lib/i18n";

const TECHNICAL_COPY = new Set([
  "Cortex",
  "Esc",
  "Groq",
  "OpenAI",
  "Tailscale",
  "Rust · Tauri · Svelte",
  "Omarchy ·",
  "age",
  "cortex",
  "ffmpeg",
  "mpv",
  "rclone",
  "sync",
  "syncd",
  "yt-dlp",
]);

function isTechnicalCopy(source: string): boolean {
  return TECHNICAL_COPY.has(source)
    || /^(?:https?:\/\/|~\/|\/)[^ ]+$/.test(source)
    || /^[A-Z][A-Z0-9_-]+(?:=|[-])?…?$/.test(source)
    || /^[A-Z][A-Z0-9_]+=/.test(source)
    || /^(?:[\w.-]+\/)+[\w.-]+$/.test(source)
    || /^[\w.-]+\/$/.test(source)
    || /^[\w.-]+:[\w.-]+$/.test(source)
    || /^[a-z0-9]+(?:[-/.][a-z0-9]+)+$/.test(source)
    || /^[a-z0-9]{30,}$/.test(source)
    || /^…?[\w.-]+\.[a-z]{2,}$/.test(source)
    || /^(?:docker compose|git pull|sudo |rclone |age-keygen)/.test(source)
    || /^(?:[\w.-]+:)?[\w.-]+\.(?:com|edu|net|org)(?:\/\S*)?$/.test(source);
}

function hasSkipAttribute(value: Record<string, unknown>): boolean {
  const attributes = value.attributes;
  return Array.isArray(attributes) && attributes.some((attribute) => {
    return attribute
      && typeof attribute === "object"
      && "name" in attribute
      && attribute.name === "data-i18n-skip";
  });
}

export function svelteFilesUnder(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return svelteFilesUnder(path);
    return entry.isFile() && entry.name.endsWith(".svelte") ? [path] : [];
  });
}

export function findUntranslatedStaticCopy(files = svelteFilesUnder("src")): string[] {
  const untranslated: string[] = [];
  for (const file of files) {
    const ast = parse(readFileSync(file, "utf8"), { filename: file }) as any;
    const seen = new Set<object>();
    const visit = (value: any, parent: any = null, skipped = false) => {
      if (!value || typeof value !== "object" || seen.has(value)) return;
      seen.add(value);
      const skipChildren = skipped || hasSkipAttribute(value);
      if (!skipChildren && value.type === "Text" && typeof value.data === "string") {
        const inAttribute = parent?.type === "Attribute" || parent?.type?.endsWith?.("Directive");
        const translatedAttribute = inAttribute && ["title", "placeholder", "aria-label"].includes(parent.name);
        if (!inAttribute || translatedAttribute) {
          const source = value.data.trim().replace(/\s+/g, " ");
          if (
            /[A-Za-z]{2}/.test(source)
            && !isTechnicalCopy(source)
            && translateText(source, "zh-CN") === source
          ) {
            untranslated.push(`${file}: ${source}`);
          }
        }
      }
      for (const [key, child] of Object.entries(value)) {
        if (key === "parent" || key === "metadata") continue;
        if (Array.isArray(child)) child.forEach((item) => visit(item, value, skipChildren));
        else visit(child, value, skipChildren);
      }
    };
    visit(ast.html);
  }
  return untranslated.sort();
}

if (import.meta.main) {
  const findings = findUntranslatedStaticCopy();
  const counts = new Map<string, number>();
  for (const entry of findings) {
    const file = entry.slice(0, entry.indexOf(":"));
    counts.set(file, (counts.get(file) ?? 0) + 1);
  }
  const byFile = [...counts]
    .map(([file, count]) => `${file}: ${count}`)
    .join("\n");
  console.log(`${findings.length} untranslated static strings across ${svelteFilesUnder("src").length} Svelte files`);
  if (byFile) console.log(byFile);
}
