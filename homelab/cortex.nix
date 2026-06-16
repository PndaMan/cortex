# homelab/cortex.nix — declarative NixOS deployment of the Cortex homelab backend.
#
# Mirrors homelab/docker-compose.yml (a single Caddy proxy on :8080 fronting
# SearXNG / Whisper / WebDAV-sync / optional Ollama) using NixOS'
# virtualisation.oci-containers with the podman backend and rootful system units.
#
# Why this over the imperative `podman` + `systemctl --user` deploy:
#   * survives reboots and needs no `loginctl enable-linger` (system units),
#   * one `nixos-rebuild switch` reproduces the whole stack,
#   * secrets come from sops-nix instead of being typed into env/compose.
#
# REACHABILITY: served on BOTH the LAN (http://<host>:8080 on the LAN interface)
# and the tailnet (https://<magicdns>.ts.net via `tailscale serve`). Nothing is
# exposed to the public internet. Cortex auto-picks local -> Tailscale.
#
# DATA: all state lives in named podman volumes (cortex-*-{models,data}) which
# persist across rebuilds. Moving off the OLD rootless deploy is a one-time copy
# (homelab/migrate-cortex-data.sh). The WebDAV sync library (your data) is also
# backed up nightly to /var/backups/cortex.
#
# WIRING (on aether):
#   1. import ./cortex.nix from configuration.nix (or the relevant host module).
#   2. Add two sops secrets (declared below):
#        cortex/sync-password   – REUSE the existing value from
#                                 ~/cortex-homelab/.sync-password so current
#                                 Cortex clients keep syncing without re-auth.
#        cortex/searxng-secret  – any `openssl rand -hex 32`.
#   3. nixos-rebuild switch.
#   4. `sudo tailscale up` once (auth), then `tailscale serve status` to confirm.
#   5. If carrying data over from the old stack: run homelab/migrate-cortex-data.sh.

{ config, pkgs, lib, ... }:

let
  enableOllama  = true;                      # compose `--profile ollama`
  enableBackups = true;                      # nightly export of the WebDAV sync library
  tailscaleServe = true;                     # expose :8080 over the tailnet as HTTPS
  lanInterface  = "eno2";                    # interface to expose :8080 on (LAN)
  net = "cortex";                            # shared podman network for service DNS
  whisperModel = "Systran/faster-whisper-base.en";

  # Caddy reverse-proxy config — identical routing to homelab/Caddyfile.
  caddyfile = pkgs.writeText "Caddyfile" ''
    :8080 {
      handle_path /searxng/* { reverse_proxy searxng:8080 }
      handle_path /whisper/* { reverse_proxy whisper:8000 }
      handle_path /ollama/*  { reverse_proxy ollama:11434 }
      handle_path /sync/*    { reverse_proxy sync:80 }
      handle / {
        respond "Cortex homelab is up. Services: /searxng /whisper /ollama /sync" 200
      }
    }
  '';

  podmanUnit = name: "podman-${name}.service";
  containerNames =
    [ "cortex-proxy" "cortex-searxng" "cortex-whisper" "cortex-sync" ]
    ++ lib.optional enableOllama "cortex-ollama";
in
{
  #### Secrets (sops-nix) #####################################################
  # The encrypted values live in your sops file; these only declare them.
  sops.secrets."cortex/sync-password" = { };
  sops.secrets."cortex/searxng-secret" = { };

  # Env file for the WebDAV container (bytemark/webdav reads USERNAME/PASSWORD).
  sops.templates."cortex-sync.env".content = ''
    AUTH_TYPE=Basic
    USERNAME=cortex
    PASSWORD=${config.sops.placeholder."cortex/sync-password"}
  '';

  # SearXNG settings.yml with the secret_key injected. JSON output MUST stay on
  # or Cortex's search requests get a 403.
  sops.templates."searxng-settings.yml".content = ''
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

  #### Container stack ########################################################
  # Data lives in NAMED volumes (podman manages ownership and they survive
  # `nixos-rebuild switch` and container recreation). See migrate-cortex-data.sh
  # to bring data over from the old rootless deploy.
  virtualisation.oci-containers = {
    backend = "podman";
    containers = {
      cortex-proxy = {
        image = "caddy:2-alpine";
        ports = [ "8080:8080" ];             # the ONLY published port (LAN + loopback)
        volumes = [
          "${caddyfile}:/etc/caddy/Caddyfile:ro"
          "cortex-caddy-data:/data"
        ];
        dependsOn = [ "cortex-searxng" "cortex-whisper" "cortex-sync" ];
        extraOptions = [ "--network=${net}" "--network-alias=proxy" ];
      };

      cortex-searxng = {
        image = "searxng/searxng:latest";
        volumes = [
          "${config.sops.templates."searxng-settings.yml".path}:/etc/searxng/settings.yml:ro"
        ];
        environment.SEARXNG_BASE_URL = "http://localhost:8080/searxng/";
        extraOptions = [
          "--network=${net}" "--network-alias=searxng"
          "--cap-drop=ALL" "--cap-add=CHOWN" "--cap-add=SETGID" "--cap-add=SETUID"
        ];
      };

      cortex-whisper = {
        # Speaches: OpenAI-compatible ASR. Swap to :latest-cuda on a GPU host.
        image = "ghcr.io/speaches-ai/speaches:latest-cpu";
        volumes = [ "cortex-whisper-models:/home/ubuntu/.cache/huggingface" ];
        environment.WHISPER__MODEL = whisperModel;
        extraOptions = [ "--network=${net}" "--network-alias=whisper" ];
      };

      cortex-sync = {
        # WebDAV target for Cortex live sync (last-write-wins DB snapshot).
        image = "bytemark/webdav:latest";
        environmentFiles = [ config.sops.templates."cortex-sync.env".path ];
        volumes = [ "cortex-sync-data:/var/lib/dav" ];
        extraOptions = [ "--network=${net}" "--network-alias=sync" ];
      };
    } // lib.optionalAttrs enableOllama {
      cortex-ollama = {
        image = "ollama/ollama:latest";
        volumes = [ "cortex-ollama-models:/root/.ollama" ];
        extraOptions = [ "--network=${net}" "--network-alias=ollama" ];
        # GPU: add "--device=nvidia.com/gpu=all" (needs the NVIDIA container toolkit).
      };
    };
  };

  #### Shared podman network #################################################
  # oci-containers does not manage networks; create one so Caddy can resolve the
  # upstreams by their --network-alias names. Ordered before every container.
  systemd.services.init-cortex-network = {
    description = "Create the cortex podman network";
    after = [ "network.target" ];
    before = map podmanUnit containerNames;
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      ${pkgs.podman}/bin/podman network exists ${net} \
        || ${pkgs.podman}/bin/podman network create ${net}
    '';
  };

  #### Tailscale exposure ####################################################
  services.tailscale.enable = true;          # `sudo tailscale up` once to authenticate

  # Publish the proxy on the tailnet as HTTPS (https://<magicdns>.ts.net). serve
  # proxies from loopback, so NO port needs opening on tailscale0. Requires the
  # node up and HTTPS/MagicDNS enabled in the tailnet admin console. Idempotent.
  # (CLI syntax has varied across versions; confirm with `tailscale serve status`.)
  systemd.services.cortex-tailscale-serve = lib.mkIf tailscaleServe {
    description = "Serve Cortex homelab (:8080) over Tailscale as HTTPS";
    after = [ "tailscaled.service" "podman-cortex-proxy.service" ];
    wants = [ "tailscaled.service" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      ${pkgs.tailscale}/bin/tailscale serve --bg --https=443 http://127.0.0.1:8080 || true
    '';
  };

  #### Nightly backup of the WebDAV sync library #############################
  systemd.tmpfiles.rules = lib.mkIf enableBackups [
    "d /var/backups/cortex 0750 root root -"
  ];
  systemd.services.cortex-backup = lib.mkIf enableBackups {
    description = "Back up the Cortex WebDAV sync library (cortex-sync-data)";
    after = [ "podman-cortex-sync.service" ];
    path = [ pkgs.podman pkgs.coreutils pkgs.findutils ];
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
  # LAN: only the proxy port, only on the LAN interface (HTTP). Tailscale is
  # served as HTTPS by the unit above (no tailscale0 port needed).
  networking.firewall.interfaces.${lanInterface}.allowedTCPPorts = [ 8080 ];
  # Prefer plain HTTP over the tailnet instead of `tailscale serve`? Set
  # tailscaleServe = false and uncomment:
  #   networking.firewall.interfaces."tailscale0".allowedTCPPorts = [ 8080 ];
}
