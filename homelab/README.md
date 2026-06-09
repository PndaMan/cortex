# Cortex homelab backend

Optional self-hosted services Cortex can offload to. **Cortex works fully without
any of this** — it's for people who want web-search diagrams, keyless local models,
or transcription without installing a Python toolchain on their laptop.

| Service | What it gives Cortex | Default port |
|---------|----------------------|--------------|
| **SearXNG** | Diagrams/images in cheatsheets + web-enriched chat | `8080` |
| **Whisper** (Speaches) | Lecture transcription, OpenAI-compatible | `9009` |
| **Sync** (WebDAV) | Live sync — auto-store your library, fetch it on launch | `9010` |
| **Ollama** *(optional)* | Local LLM + embeddings, no API key | `11434` |

## Quick start

```bash
cd homelab
# (recommended) set a SearXNG secret first:
sed -i "s/change-me-openssl-rand-hex-32/$(openssl rand -hex 32)/" searxng/settings.yml

docker compose up -d                    # SearXNG + Whisper
docker compose --profile ollama up -d   # also bring up Ollama
```

Then in Cortex → **Settings → Integrations**, point each service at this host and
hit **Test**:

- **SearXNG** → `http://<host>:8080`
- **Remote transcription (Whisper)** → `http://<host>:9009`
- **Live sync (WebDAV)** → `http://<host>:9010` (user `cortex`, the password you set)
- **Local models (Ollama)** → `http://<host>:11434`

> **Live sync** keeps one library across devices: each device pushes a snapshot
> after changes and pulls a newer one on launch (last-write-wins). Change the
> WebDAV `PASSWORD` in `docker-compose.yml` before exposing it.

`<host>` is `localhost` if you run Cortex on the same machine, otherwise the
homelab box's LAN/VPN IP or hostname.

## Exposing it

Run it locally, or reach it from anywhere via **one** of:

### A VPN (simplest, recommended)
Install [Tailscale](https://tailscale.com) / [Netbird](https://netbird.io) /
WireGuard on the homelab host and your devices. Point Cortex at the host's VPN
IP (e.g. `http://100.x.y.z:8080`). Nothing is exposed to the public internet.

### A reverse proxy with TLS
Put Caddy/Traefik/nginx in front and map subdomains to the ports. Example Caddy:

```caddy
searxng.example.com { reverse_proxy localhost:8080 }
whisper.example.com { reverse_proxy localhost:9009 }
ollama.example.com  { reverse_proxy localhost:11434 }
```

Then use the `https://…` URLs in Cortex. If you expose SearXNG publicly, also set
`SEARXNG_BASE_URL` (in `docker-compose.yml`) to its public URL and keep
`server.limiter` in mind for abuse protection.

## Notes

- **GPU:** swap the Whisper image to `…:latest-cuda` and uncomment the Ollama
  `deploy.resources` block (needs the NVIDIA container toolkit).
- **Whisper model:** change `WHISPER__MODEL` to a larger model (e.g.
  `Systran/faster-whisper-large-v3`) for better accuracy at the cost of speed.
- **Pull an Ollama model** after first start:
  `docker exec -it cortex-ollama ollama pull nomic-embed-text` (embeddings) and a
  chat model like `llama3.1`.
- **SearXNG JSON** is pre-enabled in `searxng/settings.yml` — without it Cortex
  gets a 403.

## Deploying on a homelab host (agent-ready checklist)

Everything needed is this directory — copy it to the host and run it there:

```bash
# on the homelab host (needs docker + the compose plugin)
scp -r homelab/ <host>:~/cortex-homelab && ssh <host>
cd ~/cortex-homelab
sed -i "s/change-me-openssl-rand-hex-32/$(openssl rand -hex 32)/" searxng/settings.yml
sed -i "s/change-me-sync-password/<a-real-password>/" docker-compose.yml
docker compose up -d                    # SearXNG + Whisper + Sync
docker compose --profile ollama up -d   # …plus Ollama, if wanted
```

Verify from any machine on the same VPN/LAN:

```bash
curl -s 'http://<host>:8080/search?q=test&format=json' | head -c 200   # SearXNG (must NOT be a 403)
curl -s http://<host>:9009/v1/models                                    # Whisper
curl -su cortex:<password> -X PROPFIND http://<host>:9010/              # WebDAV sync
```

Then in Cortex → Settings → Integrations set: SearXNG `http://<host>:8080`,
remote transcription `http://<host>:9009`, live sync `http://<host>:9010` with the
WebDAV credentials. Keep it reachable only over your VPN (Tailscale/Netbird/
WireGuard) or behind a TLS reverse proxy — never bare on the internet.

To trial the stack on a workstation first without exposing anything on the LAN,
`docker-compose.local.yml` overrides every port to 127.0.0.1:

```bash
docker compose -f docker-compose.yml -f docker-compose.local.yml up -d
```
