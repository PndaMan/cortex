# Packaging & distribution

Everything points back to **GitHub Releases** (built automatically by
[`.github/workflows/release.yml`](../.github/workflows/release.yml) on every
`v*` tag). The other channels just wrap those release assets.

## Where to publish — start here

| Channel | Effort | Signing needed? | Status |
|---|---|---|---|
| **GitHub Releases** | ✅ automated | No | Done — CI attaches `.deb`/`.rpm`/`.dmg`/`.exe` |
| **AUR** (`cortex-bin`) | Low | No | `aur/PKGBUILD` ready — best fit, you're on Arch |
| **Scoop** (Windows, own bucket) | Low | No | manifest below |
| **Homebrew** (own tap, Cask) | Low | Recommended | `homebrew/cortex.rb` ready (Gatekeeper prompt until notarized) |
| **winget** (Windows) | Medium | Recommended | submit to `microsoft/winget-pkgs` |
| **Flathub** / **Snap** | High | No / Store | defer until there's traction |
| Official `homebrew-cask` | High | **Required** (notarized) | defer until signed + popular |

**Recommended first wave (no approval gates):** AUR `cortex-bin` + a Scoop bucket now; a Homebrew **tap** and **winget** next (they work unsigned but show a one-time security prompt until you sign — see [`../SIGNING.md`](../SIGNING.md)). Flathub/Snap/official-cask later.

## AUR (`cortex-bin`)

```sh
# one-time: add your SSH key at https://aur.archlinux.org/account
git clone ssh://aur@aur.archlinux.org/cortex-bin.git
cd cortex-bin
cp /path/to/cortex/packaging/aur/PKGBUILD .
# set the real checksum:
updpkgsums                       # or: sha256sum the .deb and paste it in
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO && git commit -m "cortex-bin 1.0.1" && git push
```

## Homebrew tap

```sh
# create a repo named `homebrew-cortex`, then:
mkdir Casks && cp /path/to/cortex/packaging/homebrew/cortex.rb Casks/
# fill in both sha256 (shasum -a 256 *.dmg), commit, push
# users install with:  brew install --cask pndaman/cortex/cortex
```

## Scoop (Windows) — your own bucket

Create a repo `scoop-cortex` with `bucket/cortex.json`:

```json
{
  "version": "1.0.1",
  "description": "Local-first NotebookLM alternative — a desktop study OS",
  "homepage": "https://github.com/PndaMan/cortex",
  "license": "Apache-2.0",
  "architecture": {
    "64bit": {
      "url": "https://github.com/PndaMan/cortex/releases/download/v1.0.1/Cortex_1.0.1_x64-setup.exe",
      "hash": "REPLACE_WITH_EXE_SHA256"
    }
  },
  "installer": { "args": ["/S"] },
  "shortcuts": [["Cortex.exe", "Cortex"]]
}
```
Users: `scoop bucket add cortex https://github.com/PndaMan/scoop-cortex && scoop install cortex`.

## winget

Easiest with [`wingetcreate`](https://github.com/microsoft/winget-create):

```sh
wingetcreate new https://github.com/PndaMan/cortex/releases/download/v1.0.1/Cortex_1.0.1_x64-setup.exe
# it fills the installer hash + walks you through the manifest, then:
wingetcreate submit   # opens a PR to microsoft/winget-pkgs
```
Package id suggestion: `PndaMan.Cortex`. Unsigned is accepted, but SmartScreen warns users until the `.exe` is code-signed.

> **Checksums** in `aur/PKGBUILD` and `homebrew/cortex.rb` are placeholders — fill them from the actual v1.0.1 release assets once the build finishes.
