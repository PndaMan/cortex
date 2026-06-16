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
# WIRING (on aether):
#   1. import ./cortex.nix from configuration.nix (or the relevant host module).
#   2. Add two sops secrets (declared below):
#        cortex/sync-password   – REUSE the existing value from
#                                 ~/cortex-homelab/.sync-password so current
#                                 Cortex clients keep syncing without re-auth.
#        cortex/searxng-secret  – any `openssl rand -hex 32`.
#   3. nixos-rebuild switch.
#
# DATA NOTE: the current stack runs as ROOTLESS podman under user `aidan`
# (volumes under ~/.local/share/containers). These system units use ROOTFUL
# podman (/var/lib/containers), so the old volumes are NOT reused — Whisper/Ollama
# models re-download and the WebDAV sync DB re-pushes on the next client sync
# (last-write-wins). Migrate with `podman volume export/import` if you want to
# avoid the re-download.

{ config, pkgs, lib, ... }:

let
  enableOllama = true;                       # compose `--profile ollama`
  lanInterface = "eno2";                     # interface to expose :8080 on
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
  virtualisation.oci-containers = {
    backend = "podman";
    containers = {
      cortex-proxy = {
        image = "caddy:2-alpine";
        ports = [ "8080:8080" ];             # the ONLY published port
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

  #### Firewall ##############################################################
  # Only the proxy port, and only on the LAN interface. Nothing is public:
  # reach it over Tailscale/NetBird or a TLS reverse proxy.
  networking.firewall.interfaces.${lanInterface}.allowedTCPPorts = [ 8080 ];
}
