# Code signing

The release workflow already has the signing hooks wired in — they stay inert
until you add the GitHub **Actions secrets** below, so unsigned builds keep
working in the meantime. Signing happens **in CI on the runners**, so you don't
need a Mac or a Windows machine locally.

---

## macOS (Developer ID + notarization)

Removes the "unidentified developer" Gatekeeper block. You have the Apple
Developer account; here's how to get the certificate **without a Mac**.

### 1. Make a Developer ID cert with `openssl` (no Keychain needed)

```sh
# Generate a key + certificate signing request
openssl genrsa -out devid.key 2048
openssl req -new -key devid.key -out devid.csr \
  -subj "/CN=Cortex Developer ID/emailAddress=aidanmcconnon210@gmail.com"
```

1. Go to **developer.apple.com → Certificates → +** → **Developer ID Application**.
2. Upload `devid.csr`, download the resulting `developerID_application.cer`.
3. Bundle the cert + key into a password-protected `.p12`:

```sh
openssl x509 -inform DER -in developerID_application.cer -out devid.crt
openssl pkcs12 -export -inkey devid.key -in devid.crt \
  -out cortex-devid.p12 -name "Developer ID Application"   # set a password here
base64 -w0 cortex-devid.p12 > cortex-devid.p12.b64          # macOS: base64 -i …
```

### 2. Add these GitHub Actions secrets (Settings → Secrets → Actions)

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | contents of `cortex-devid.p12.b64` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password from step 1 |
| `APPLE_ID` | your Apple ID email |
| `APPLE_PASSWORD` | an **app-specific password** (appleid.apple.com → Sign-In & Security) |
| `APPLE_TEAM_ID` | your 10-char Team ID (developer.apple.com → Membership) |
| `KEYCHAIN_PASSWORD` | any random string (CI keychain password) |

Next tagged release auto-signs **and notarizes** the `.dmg`/`.app` — `tauri-action` does the notarization once `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` are present.

---

## Windows (code signing)

Removes the SmartScreen "unknown publisher" warning. Traditional OV/EV certs now
require a hardware token (painful in CI), so use a cloud signer:

- **SignPath.io** — **free for open-source projects** (this repo qualifies).
  Cloud signing with a GitHub Action. Easiest path.
- **Azure Trusted Signing** — ~pay-as-you-go (~$10/mo), CI-friendly, but identity
  validation takes a few days.

Both run as a post-build step that signs the `.exe`. Tell me which you pick and
I'll wire the action + the `bundle.windows.signCommand` into the workflow. A
**self-signed** cert does *not* help — SmartScreen ignores it.

---

## Updater signing (separate, free — only if you add auto-update)

Tauri's auto-updater needs its own minisign key (unrelated to the OS certs):

```sh
bunx tauri signer generate -w ~/.tauri/cortex.key
```

Put the **public** key in `tauri.conf.json` (`plugins.updater.pubkey`) and the
**private** key + password in `TAURI_SIGNING_PRIVATE_KEY` /
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets. Not needed unless/until the
auto-updater is enabled.
