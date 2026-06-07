# Cortex homelab backend

Optional self-hosted services Cortex can offload to. **Cortex works fully without
any of this** — it's for people who want web-search diagrams, keyless local models,
or transcription without installing a Python toolchain on their laptop.

| Service | What it gives Cortex | Default port |
|---------|----------------------|--------------|
| **SearXNG** | Diagrams/images in cheatsheets + web-enriched chat | `8080` |
| **Whisper** (Speaches) | Lecture transcription, OpenAI-compatible | `9009` |
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
- **Local models (Ollama)** → `http://<host>:11434`

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
