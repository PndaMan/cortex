# homelab/cortex.nix — declarative NixOS deployment of the Cortex homelab backend.
#
# Mirrors homelab/docker-compose.yml (one Caddy proxy fronting SearXNG / Whisper
# / WebDAV-sync / optional Ollama) using NixOS' virtualisation.oci-containers
# (podman). All services run in a SINGLE podman POD, so Caddy reaches them over
# localhost — deliberately NOT a podman DNS network, because podman's aardvark-dns
# binds :53 on the bridge gateway and fails on any host already running a resolver
# there (AdGuard / Pi-hole / dnsmasq / systemd-resolved). The pod sidesteps that.
#
# Why this over the imperative `podman` + `systemctl --user` deploy:
#   * survives reboots and needs no `loginctl enable-linger` (system units),
#   * one `nixos-rebuild switch` reproduces the whole stack,
#   * secrets come from sops-nix instead of being typed into env/compose.
#
# REACHABILITY: served on the LAN (http://<host>:8080) and the tailnet
# (https://<magicdns>.ts.net via `tailscale serve`). Nothing is public.
#
# DATA: named podman volumes (cortex-*-{models,data}) persist across rebuilds; the
# WebDAV library is also backed up nightly to /var/backups/cortex. To carry data
# over from an old rootless deploy: homelab/migrate-cortex-data.sh.
#
# WIRING:
#   1. import ./cortex.nix from your host config.
#   2. add two sops secrets: cortex/sync-password, cortex/searxng-secret.
#   3. nixos-rebuild switch; `sudo tailscale up` once if using Tailscale.
#
# NOTE: 8080 is a busy port — if something else owns it on your host (e.g.
# Vaultwarden), change hostPort below. (Validated live on a NixOS host 2026-06-16.)

{ config, pkgs, lib, ... }:

let
  enableOllama   = true;                     # compose `--profile ollama`
  enableBackups  = true;                     # nightly export of the WebDAV library
  tailscaleServe = true;                     # expose over the tailnet as HTTPS
  lanInterface   = "eno2";                   # interface to open hostPort on (LAN)
  hostPort  = 8080;                          # the single published Homelab URL port
  proxyPort = 8088;                          # Caddy's port INSIDE the pod (8080 is SearXNG's)
  whisperModel = "deepdml/faster-whisper-large-v3-turbo-ct2";
  # OPTIONAL access token: bcrypt hash of the token every request (except /sync,
  # which has its own WebDAV credentials) must present as Basic auth user
  # "cortex". Generate with `caddy hash-password --plaintext 'your-token'` (or
  # `docker run --rm caddy:2-alpine caddy hash-password --plaintext 'your-token'`)
  # and paste the SAME plaintext token into Cortex → Settings → Integrations →
  # Homelab → Access token. REQUIRED before exposing the proxy on a public URL —
  # unauthenticated whisper/ollama/searxng/ingest on the open internet means
  # anyone can burn your compute. Empty string = auth disabled (LAN/Tailscale
  # only setups).
  cortexTokenHash = "";

  # In a pod all containers share one netns, so Caddy proxies to localhost. Note
  # the Caddyfile block syntax: `{` must end the line — inline `{ directive }` is
  # rejected by Caddy ("Unexpected next token after '{' on same line").
  caddyfile = pkgs.writeText "cortex-Caddyfile" ''
    :${toString proxyPort} {
      ${lib.optionalString (cortexTokenHash != "") ''
      @cortex_protected {
        not path /sync/*
      }
      basic_auth @cortex_protected {
        cortex ${cortexTokenHash}
      }
      ''}
      handle_path /searxng/* {
        reverse_proxy localhost:8080
      }
      handle_path /whisper/* {
        reverse_proxy localhost:8000
      }
      handle_path /ollama/* {
        reverse_proxy localhost:11434
      }
      handle_path /sync/* {
        reverse_proxy localhost:80
      }
      handle_path /ingest/* {
        reverse_proxy localhost:9998
      }
      handle / {
        respond "Cortex homelab is up. Services: /searxng /whisper /ollama /sync /ingest" 200
      }
    }
  '';

  podmanBin = "${config.virtualisation.podman.package}/bin/podman";
  containerNames =
    [ "cortex-proxy" "cortex-searxng" "cortex-whisper" "cortex-sync" "cortex-ingest" ]
    ++ lib.optional enableOllama "cortex-ollama";
  inPod = [ "--pod=cortex" ];
in
{
  #### Secrets (sops-nix) #####################################################
  # The encrypted values live in your sops file; these only declare them.
  sops.secrets."cortex/sync-password" = { };
  sops.secrets."cortex/searxng-secret" = { };

  # Env file for the WebDAV container (read by podman as root → 0400 is fine).
  sops.templates."cortex-sync.env".content = ''
    AUTH_TYPE=Basic
    USERNAME=cortex
    PASSWORD=${config.sops.placeholder."cortex/sync-password"}
  '';

  # SearXNG settings.yml with the secret_key injected. 0444 so the in-container
  # searxng user (rootful podman = direct uid) can read it. JSON MUST stay on
  # or Cortex's search requests get a 403.
  sops.templates."searxng-settings.yml" = {
    mode = "0444";
    content = ''
      use_default_settings: true
      server:
        secret_key: "${config.sops.placeholder."cortex/searxng-secret"}"
        limiter: false
        image_proxy: true
      search:
        formats:
          - html
          - json
      ui:
        static_use_hash: true
    '';
  };

  #### Container stack — all in the `cortex` pod #############################
  # Data lives in NAMED volumes (podman manages ownership; they survive rebuilds
  # and container recreation). See migrate-cortex-data.sh to bring data over.
  virtualisation.oci-containers = {
    backend = "podman";
    containers = {
      cortex-proxy = {
        image = "docker.io/library/caddy:2-alpine";
        # No `ports` — the pod owns the published port (see init-cortex-pod).
        volumes = [
          "${caddyfile}:/etc/caddy/Caddyfile:ro"
          "cortex-caddy-data:/data"
        ];
        dependsOn = [ "cortex-searxng" "cortex-whisper" "cortex-sync" "cortex-ingest" ];
        extraOptions = inPod;
      };

      cortex-searxng = {
        image = "docker.io/searxng/searxng:latest";
        volumes = [
          "${config.sops.templates."searxng-settings.yml".path}:/etc/searxng/settings.yml:ro"
        ];
        environment.SEARXNG_BASE_URL = "http://localhost:${toString hostPort}/searxng/";
        extraOptions = inPod ++ [
          "--cap-drop=ALL" "--cap-add=CHOWN" "--cap-add=SETGID" "--cap-add=SETUID"
        ];
      };

      cortex-whisper = {
        # Speaches: OpenAI-compatible ASR. Swap to :latest-cuda on a GPU host.
        image = "ghcr.io/speaches-ai/speaches:latest-cpu";
        volumes = [ "cortex-whisper-models:/home/ubuntu/.cache/huggingface" ];
        environment.WHISPER__MODEL = whisperModel;
        extraOptions = inPod;
      };

      cortex-sync = {
        # WebDAV target for Cortex live sync (last-write-wins DB snapshot).
        image = "docker.io/bytemark/webdav:latest";
        environmentFiles = [ config.sops.templates."cortex-sync.env".path ];
        volumes = [ "cortex-sync-data:/var/lib/dav" ];
        extraOptions = inPod;
      };

      cortex-ingest = {
        # Apache Tika — document → text (PDF/DOCX/PPTX/legacy + OCR of scanned
        # pages via the `-full` tag's Tesseract). Cortex mobile offloads parsing
        # here (a phone can't run poppler/libreoffice). Reached at /ingest/tika.
        image = "docker.io/apache/tika:latest-full";
        extraOptions = inPod;
      };
    } // lib.optionalAttrs enableOllama {
      cortex-ollama = {
        image = "docker.io/ollama/ollama:latest";
        volumes = [ "cortex-ollama-models:/root/.ollama" ];
        extraOptions = inPod;
        # GPU: add "--device=nvidia.com/gpu=all" (needs the NVIDIA container toolkit).
      };
    };
  };

  #### Pod (shared netns, DNS disabled → no aardvark/:53 clash) ##############
  # oci-containers does not manage pods; create one (and a DNS-free network for
  # outbound NAT) before the containers, which join it via `--pod=cortex`.
  systemd.services.init-cortex-pod = {
    description = "Create the cortex podman pod";
    after = [ "network.target" "podman.service" ];
    before = map (n: "podman-${n}.service") containerNames;
    requiredBy = map (n: "podman-${n}.service") containerNames;
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    script = ''
      ${podmanBin} network exists cortex-net || ${podmanBin} network create --disable-dns cortex-net
      ${podmanBin} pod exists cortex || ${podmanBin} pod create --name cortex \
        --network cortex-net -p ${toString hostPort}:${toString proxyPort}
    '';
  };

  #### Tailscale exposure ####################################################
  services.tailscale.enable = true;          # `sudo tailscale up` once to authenticate

  # Publish the proxy on the tailnet as HTTPS (https://<magicdns>.ts.net). serve
  # proxies from loopback, so NO port needs opening on tailscale0. Requires the
  # node up and HTTPS/MagicDNS enabled in the tailnet admin console. Idempotent.
  # (CLI syntax has varied across versions; confirm with `tailscale serve status`.)
  systemd.services.cortex-tailscale-serve = lib.mkIf tailscaleServe {
    description = "Serve Cortex homelab over Tailscale as HTTPS";
    after = [ "tailscaled.service" "podman-cortex-proxy.service" ];
    wants = [ "tailscaled.service" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    script = ''
      ${pkgs.tailscale}/bin/tailscale serve --bg --https=443 http://127.0.0.1:${toString hostPort} || true
    '';
  };

  #### Nightly backup of the WebDAV sync library ############################
  systemd.tmpfiles.rules = lib.mkIf enableBackups [
    "d /var/backups/cortex 0750 root root -"
  ];
  systemd.services.cortex-backup = lib.mkIf enableBackups {
    description = "Back up the Cortex WebDAV sync library (cortex-sync-data)";
    after = [ "podman-cortex-sync.service" ];
    path = [ config.virtualisation.podman.package pkgs.coreutils pkgs.findutils ];
    serviceConfig = { Type = "oneshot"; };
    script = ''
      set -euo pipefail
      stamp=$(date +%Y%m%d-%H%M%S)
      podman volume export cortex-sync-data -o "/var/backups/cortex/sync-data-$stamp.tar"
      # keep the 14 most recent exports
      ls -1t /var/backups/cortex/sync-data-*.tar | tail -n +15 | xargs -r rm -f
    '';
  };
  systemd.timers.cortex-backup = lib.mkIf enableBackups {
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnCalendar = "daily";
      Persistent = true;
      RandomizedDelaySec = "30m";
    };
  };

  #### Firewall ##############################################################
  # LAN: only the published port, only on the LAN interface. Tailscale is served
  # as HTTPS by the unit above (no tailscale0 port needed).
  networking.firewall.interfaces.${lanInterface}.allowedTCPPorts = [ hostPort ];
}
