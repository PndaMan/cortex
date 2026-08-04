# Cortex homelab backend

Optional self-hosted services Cortex can offload to. **Cortex works fully without
any of this** — it's for people who want web-search diagrams, keyless local models,
or transcription without installing a Python toolchain on their laptop.

Everything sits behind **one URL**: a small Caddy reverse proxy publishes a single
port and routes by path to each service. You give Cortex that one address and it
appends the rest.

| Service | Reached at | What it gives Cortex |
|---------|-----------|----------------------|
| **SearXNG** | `<url>/searxng` | Diagrams/images in cheatsheets + web-enriched chat |
| **WhisperX** (whisper-asr-webservice) | `<url>/whisper` | Long-form lecture transcription + speaker diarization |
| **Sync** (WebDAV) | `<url>/sync` | Live sync — auto-store your library, fetch it on launch |
| **Ingest** (Apache Tika) | `<url>/ingest` | Document → text for **mobile** (PDF/DOCX/PPTX/legacy + OCR of scanned pages) — a phone can't run poppler/libreoffice |
| **Ollama** *(optional)* | `<url>/ollama` | Local LLM + embeddings, no API key |

Only the proxy is exposed (default port `8080`); the services themselves are
internal to the compose network.

## Quick start

```bash
cd homelab
# (recommended) set a SearXNG secret first:
sed -i "s/change-me-openssl-rand-hex-32/$(openssl rand -hex 32)/" searxng/settings.yml

docker compose up -d                    # proxy + SearXNG + Whisper + Sync
docker compose --profile ollama up -d   # also bring up Ollama
```

Then in Cortex → **Settings → Integrations → Homelab URL**, enter the single
address and hit **Test**:

```
http://<host>:8080
```

`<host>` is `localhost` if you run Cortex on the same machine, otherwise the
homelab box's LAN/VPN IP or hostname. That's it — search, transcription, sync, and
(if enabled) Ollama all work off that one URL. The per-service override fields are
only for running a service somewhere else.

> **Auto local → Tailscale → public.** Add a Tailscale and/or public URL in the
> same panel and Cortex uses the first reachable one, so the same device works on
> LAN, over the VPN, or from anywhere — without reconfiguring each service.

## Exposing it

Reach it from anywhere via **one** of:

### A VPN (simplest, recommended)
Install [Tailscale](https://tailscale.com) / [Netbird](https://netbird.io) /
WireGuard on the homelab host and your devices, then put the host's VPN URL
(e.g. `https://homelab.tailnet-xxxx.ts.net`) in the **Tailscale URL** field.
Nothing is exposed to the public internet.

### A reverse proxy with TLS
Front the proxy's `:8080` with your own TLS terminator and use the `https://…`
base as the **Public URL**:

```caddy
lab.example.com { reverse_proxy localhost:8080 }
```

The bundled `Caddyfile` already handles the per-service path routing — you only
need to add TLS in front of it.

## Security — read this before using a public URL

Out of the box the proxy has **no authentication**: fine on a LAN or a
Tailscale tailnet (both private), dangerous on a public URL — anyone on the
internet could use your Whisper/Ollama compute, hammer SearXNG, and feed
files to ingest. (`/sync` is the exception: the WebDAV container enforces its
own username/password, so the vault itself is always credentialed.)

To expose the homelab publicly, set an access token:

```bash
CORTEX_TOKEN='some-long-random-string' docker compose up -d
```

then paste the same token into **Cortex → Settings → Integrations → Homelab →
Access token**. Every service (except `/sync`) now requires it; Cortex sends
it automatically on every request. On NixOS set `cortexTokenHash` in
`cortex.nix` (see the comment there). Also prefer HTTPS for the public URL —
put the proxy behind your TLS terminator or a Cloudflare tunnel so the token
never crosses the wire in clear.

## Notes

- **GPU:** swap the Whisper image to `…:latest-gpu` with `WHISPER_DEVICE=cuda`,
  and uncomment the Ollama `deploy.resources` block (needs the NVIDIA container
  toolkit). An hour-long lecture transcribes in a couple of minutes on GPU.
- **Whisper model:** set `WHISPER_MODEL` (default `distil-large-v3` — near
  large-v3 accuracy on English at a fraction of the compute). Tiers:
  `small` (weak CPU) → `distil-large-v3` (default) → `large-v3` (GPU / maximum
  accuracy). Models lazy-download on first use into the `whisper-models` volume.
- **Speaker labels (diarization):** transcripts come back as
  `Speaker 1: … / Speaker 2: …` when a Hugging Face token is configured:
  create a free account, make a **read** token (Settings → Access Tokens),
  accept the terms on BOTH gated models —
  [pyannote/speaker-diarization-3.1](https://huggingface.co/pyannote/speaker-diarization-3.1)
  and [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0) —
  then start the stack with `HF_TOKEN=hf_xxx docker compose up -d`. Without a
  token, transcription still works and diarization is silently skipped.
- **Long lectures:** WhisperX's VAD-batched pipeline is built for hour-plus
  audio — no more transcripts cutting off near the end; the app also allows the
  request up to three hours before timing out.
- **Pull an Ollama model** after first start:
  `docker exec -it cortex-ollama ollama pull nomic-embed-text` (embeddings) and a
  chat model like `llama3.1`.
- **SearXNG JSON** is pre-enabled in `searxng/settings.yml` — without it Cortex
  gets a 403.
- **Change the WebDAV `PASSWORD`** in `docker-compose.yml` before exposing sync.

## Deploying on a homelab host (agent-ready checklist)

Everything needed is this directory — copy it to the host and run it there:

```bash
# on the homelab host (needs docker + the compose plugin)
scp -r homelab/ <host>:~/cortex-homelab && ssh <host>
cd ~/cortex-homelab
sed -i "s/change-me-openssl-rand-hex-32/$(openssl rand -hex 32)/" searxng/settings.yml
sed -i "s/change-me-sync-password/<a-real-password>/" docker-compose.yml
docker compose up -d                    # proxy + SearXNG + Whisper + Sync
docker compose --profile ollama up -d   # …plus Ollama, if wanted
```

Verify the single URL routes to every service:

```bash
curl -s 'http://<host>:8080/searxng/search?q=test&format=json' | head -c 200  # SearXNG (must NOT be 403)
curl -s http://<host>:8080/whisper/openapi.json | head -c 200                  # WhisperX
curl -su cortex:<password> -X PROPFIND http://<host>:8080/sync/                 # WebDAV sync
curl -s http://<host>:8080/                                                    # liveness banner
```

Then in Cortex → Settings → Integrations set the **Homelab URL** to
`http://<host>:8080`. Keep it reachable only over your VPN (Tailscale/Netbird/
WireGuard) or behind a TLS reverse proxy — never bare on the internet.
