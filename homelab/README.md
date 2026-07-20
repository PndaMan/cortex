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
| **Whisper** (Speaches) | `<url>/whisper` | Lecture transcription, OpenAI-compatible |
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

## Notes

- **GPU:** swap the Whisper image to `…:latest-cuda` and uncomment the Ollama
  `deploy.resources` block (needs the NVIDIA container toolkit).
- **Whisper model:** the default is `deepdml/faster-whisper-large-v3-turbo-ct2`
  (near large-v3 accuracy at ~8× its speed). On a very weak CPU box drop
  `WHISPER__MODEL` to `Systran/faster-whisper-small`; with a GPU you can afford
  `Systran/faster-whisper-large-v3` for maximum accuracy.
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
curl -s http://<host>:8080/whisper/v1/models                                   # Whisper
curl -su cortex:<password> -X PROPFIND http://<host>:8080/sync/                 # WebDAV sync
curl -s http://<host>:8080/                                                    # liveness banner
```

Then in Cortex → Settings → Integrations set the **Homelab URL** to
`http://<host>:8080`. Keep it reachable only over your VPN (Tailscale/Netbird/
WireGuard) or behind a TLS reverse proxy — never bare on the internet.
