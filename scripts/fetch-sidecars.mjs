// Fetch the small external binaries Cortex shells out to and place them as
// Tauri sidecars (src-tauri/binaries/<name>-<target-triple>[.exe]) so they ship
// inside the installer — the user never has to install them by hand.
//
// Bundled here (all single-file / cleanly extractable, cross-platform):
//   • yt-dlp  — YouTube ingest + music streaming
//   • age     — encrypted backups
//   • rclone  — backup upload
// (poppler / ffmpeg / mpv are deliberately NOT bundled — see README/Trello.)
//
// Idempotent: skips a binary that's already present. Runs before `tauri dev`
// and `tauri build` (wired into beforeDev/BuildCommand) so local Linux dev keeps
// working, and runs per-platform in CI. The target triple is taken from
// SIDECAR_TARGET (set per matrix entry in CI) or the host's rustc triple.

import { execSync } from "node:child_process";
import { mkdtempSync, mkdirSync, existsSync, copyFileSync, chmodSync, rmSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(ROOT, "src-tauri", "binaries");

const AGE_VERSION = "v1.2.1";

function hostTriple() {
  try {
    const out = execSync("rustc -vV", { encoding: "utf8" });
    const m = out.match(/host:\s*(\S+)/);
    if (m) return m[1];
  } catch {}
  // Fallback from Node's platform/arch if rustc isn't on PATH.
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  if (process.platform === "win32") return `${arch}-pc-windows-msvc`;
  if (process.platform === "darwin") return `${arch}-apple-darwin`;
  return `${arch}-unknown-linux-gnu`;
}

const triple = process.env.SIDECAR_TARGET || hostTriple();
const isWin = triple.includes("windows");
const isMac = triple.includes("apple-darwin");
const isArm = triple.startsWith("aarch64");
const exe = isWin ? ".exe" : "";
const os = isWin ? "win" : isMac ? "mac" : "linux";
const arch = isArm ? "arm64" : "amd64";

// Per-tool download recipe for the resolved triple.
function recipes() {
  // yt-dlp ships ready-to-run single binaries — no extraction.
  const ytdlp = {
    name: "yt-dlp",
    archive: null,
    url: isWin
      ? "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
      : isMac
        ? "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
        : isArm
          ? "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64"
          : "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux",
  };

  // age: .tar.gz (linux/mac) or .zip (windows); binary is age/age[.exe] inside.
  const ageOs = isWin ? "windows" : isMac ? "darwin" : "linux";
  const ageExt = isWin ? "zip" : "tar.gz";
  const age = {
    name: "age",
    archive: ageExt,
    inner: `age/age${exe}`,
    url: `https://github.com/FiloSottile/age/releases/download/${AGE_VERSION}/age-${AGE_VERSION}-${ageOs}-${arch}.${ageExt}`,
  };

  // rclone: always a .zip; binary is rclone-*/rclone[.exe] inside.
  const rcloneOs = isWin ? "windows" : isMac ? "osx" : "linux";
  const rclone = {
    name: "rclone",
    archive: "zip",
    inner: `rclone${exe}`, // nested one dir deep; matched by basename
    url: `https://downloads.rclone.org/rclone-current-${rcloneOs}-${arch}.zip`,
  };

  return [ytdlp, age, rclone];
}

async function download(url, dest) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText} for ${url}`);
  const buf = Buffer.from(await res.arrayBuffer());
  const { writeFileSync } = await import("node:fs");
  writeFileSync(dest, buf);
}

// Extract `inner` (matched by basename) from a .tar.gz / .zip into `tmp`,
// returning the path to the extracted binary.
function extract(archivePath, type, tmp, innerBasename) {
  if (type === "tar.gz") {
    execSync(`tar -xzf "${archivePath}" -C "${tmp}"`, { stdio: "inherit" });
  } else if (process.platform === "win32") {
    // bsdtar (the bundled `tar`) extracts .zip on Windows.
    execSync(`tar -xf "${archivePath}" -C "${tmp}"`, { stdio: "inherit" });
  } else {
    // GNU tar can't read .zip — use unzip (present on Linux/macOS CI + dev).
    execSync(`unzip -o -q "${archivePath}" -d "${tmp}"`, { stdio: "inherit" });
  }
  // Find the binary by basename anywhere under tmp.
  const want = innerBasename;
  const stack = [tmp];
  while (stack.length) {
    const d = stack.pop();
    for (const e of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, e.name);
      if (e.isDirectory()) stack.push(p);
      else if (e.name === want) return p;
    }
  }
  throw new Error(`could not find ${want} inside ${archivePath}`);
}

async function main() {
  mkdirSync(OUT, { recursive: true });
  console.log(`[sidecars] target triple: ${triple}`);
  for (const r of recipes()) {
    const final = join(OUT, `${r.name}-${triple}${exe}`);
    if (existsSync(final)) {
      console.log(`[sidecars] ${r.name} already present — skipping`);
      continue;
    }
    console.log(`[sidecars] fetching ${r.name} ← ${r.url}`);
    const tmp = mkdtempSync(join(tmpdir(), "cortex-sidecar-"));
    try {
      if (!r.archive) {
        await download(r.url, final);
      } else {
        const ar = join(tmp, `dl.${r.archive}`);
        await download(r.url, ar);
        const innerBasename = r.inner.split("/").pop();
        const bin = extract(ar, r.archive, tmp, innerBasename);
        copyFileSync(bin, final);
      }
      if (!isWin) chmodSync(final, 0o755);
      console.log(`[sidecars] → ${final}`);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  }
  console.log("[sidecars] done");
}

main().catch((e) => {
  console.error(`[sidecars] FAILED: ${e.message}`);
  process.exit(1);
});
