<script lang="ts">
  import { app, THEMES } from "../lib/store.svelte";
  import type { Theme } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";
  import logo from "../assets/cortex-logo.png";
  import { t } from "../lib/i18n.svelte";

  let { onFinish }: { onFinish?: () => void } = $props();

  let step = $state(0);
  let apiKey = $state("");
  let theme = $state<Theme>("osaka-jade");
  let homelab = $state(false);
  let homelabEndpoint = $state("");
  let subjName = $state("");

  const steps = [t("Welcome"), t("Your key"), t("Theme"), t("Homelab"), t("First subject")];

  function next() {
    step = Math.min(steps.length - 1, step + 1);
  }

  function pickTheme(t: Theme) {
    theme = t;
    app.setTheme(t);
  }

  function finish() {
    onFinish?.();
  }

  const THEME_OPTS: { id: Theme; n: string; c: string; b: string }[] = [
    { id: "osaka-jade",  n: t("Osaka Jade"),  c: "#2dd5b7", b: "#111c18" },
    { id: "tokyo-night", n: t("Tokyo Night"),  c: "#7aa2f7", b: "#1a1b26" },
    { id: "catppuccin",  n: t("Catppuccin"),   c: "#94e2d5", b: "#1e1e2e" },
  ];

  const SUGGESTIONS = [t("Algorithms"), t("Operating Systems"), t("Statistical Inference")];
</script>

<div class="onb">
  <!-- Left rail: step list -->
  <div class="onb-rail">
    <div class="onb-brand">
      <img class="ds-logo-glyph" src={logo} alt="Cortex" /> Cortex
    </div>
    <div class="onb-steps">
      {#each steps as s, i}
        <div class={"onb-step" + (i === step ? " on" : "") + (i < step ? " done" : "")}>
          <span class="os-num">
            {#if i < step}
              <Icon name="check" size={11} />
            {:else}
              {i + 1}
            {/if}
          </span>
          {s}
        </div>
      {/each}
    </div>
    <div class="onb-skip">
      <button class="btn btn--ghost btn--sm" onclick={finish}>{t("Skip setup")}</button>
    </div>
  </div>

  <!-- Main content -->
  <div class="onb-main">
    <div class="onb-card">
      <!-- Step 0: Welcome -->
      {#if step === 0}
        <div class="onb-pane">
          <div class="eyebrow">{t("First run")}</div>
          <h1 class="read onb-h">{t("A calm place for hard subjects.")}</h1>
          <p class="onb-p read">
            {t("Cortex turns lectures, PDFs, recordings and the web into structured study material — cheatsheets you can trust, flashcards, and a chat that stays scoped to exactly what you ask. Five quick steps; you can change anything later.")}
          </p>
          <div class="onb-feats">
            <div class="onb-feat">
              <Icon name="book" size={15} color="var(--accent)" />
              <span>{t("Completeness-checked cheatsheets — nothing silently dropped")}</span>
            </div>
            <div class="onb-feat">
              <Icon name="record" size={15} color="var(--accent)" />
              <span>{t("Lecture recording & transcription, built in")}</span>
            </div>
            <div class="onb-feat">
              <Icon name="cmd" size={15} color="var(--accent)" />
              <span>{t("Keyboard-first, Helix-style modal navigation")}</span>
            </div>
          </div>
          <button class="btn btn--primary onb-cta" onclick={next}>
            {t("Get started")} <Icon name="arrowR" size={14} />
          </button>
        </div>

      <!-- Step 1: BYOK -->
      {:else if step === 1}
        <div class="onb-pane">
          <div class="eyebrow">{t("Bring your own key")}</div>
          <h1 class="read onb-h">{t("Paste your Gemini API key.")}</h1>
          <p class="onb-p read">
            {t("Cortex is BYOK — your key stays on this machine, in the OS keychain. Nothing routes through our servers. You can add Claude, OpenAI or a local Ollama model later in Settings.")}
          </p>
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="onb-label mono">GEMINI_API_KEY</label>
          <input
            class="input"
            bind:value={apiKey}
            placeholder="AIza…"
            style="font-family:var(--font-mono)"
          />
          <div class="onb-row">
            <Icon name="diamond" size={11} color="var(--fg-faint)" />
            <span class="mono faint">{t("Stored in keychain · never synced")}</span>
          </div>
          <div class="onb-actions">
            <button class="btn btn--ghost" onclick={next}>{t("I'll add it later")}</button>
            <button class="btn btn--primary" onclick={next}>{t("Continue")} <Icon name="arrowR" size={14} /></button>
          </div>
        </div>

      <!-- Step 2: Theme -->
      {:else if step === 2}
        <div class="onb-pane">
          <div class="eyebrow">{t("Appearance")}</div>
          <h1 class="read onb-h">{t("We detected your Omarchy theme.")}</h1>
          <p class="onb-p read">
            {t("Cortex re-skins live with your desktop. We found Osaka Jade — keep it, or pick another. Switching your Omarchy theme later updates Cortex automatically.")}
          </p>
          <div class="onb-themes">
            {#each THEME_OPTS as t}
              <button
                class={"onb-theme" + (theme === t.id ? " on" : "")}
                onclick={() => pickTheme(t.id)}
                style="background:{t.b}"
              >
                <span class="ot-sw" style="background:{t.c}"></span>
                <span class="ot-name" style="color:{theme === t.id ? '#fff' : '#cbd'}">{t.n}</span>
                {#if theme === t.id}
                  <span class="ot-check" style="color:{t.c}">
                    <Icon name="check" size={13} />
                  </span>
                {/if}
              </button>
            {/each}
          </div>
          <div class="onb-actions">
            <span class="mono faint">{t("Detected via Omarchy")}</span>
            <button class="btn btn--primary" onclick={next}>{t("Continue")} <Icon name="arrowR" size={14} /></button>
          </div>
        </div>

      <!-- Step 3: Homelab -->
      {:else if step === 3}
        <div class="onb-pane">
          <div class="eyebrow">{t("Optional")}</div>
          <h1 class="read onb-h">{t("Got a homelab?")}</h1>
          <p class="onb-p read">
            {t("Offload heavy jobs — Whisper transcription, large-model synthesis, backups — to a machine on your network. Cortex stays fully local without it; this just makes big jobs faster.")}
          </p>
          <button
            class={"onb-toggle" + (homelab ? " on" : "")}
            onclick={() => (homelab = !homelab)}
          >
            <span class="ot-knob"></span>
            <span class="mono">{homelab ? t("Use homelab for heavy jobs") : t("Run everything on this machine")}</span>
          </button>
          {#if homelab}
            <div class="onb-homelab">
              <!-- svelte-ignore a11y_label_has_associated_control -->
              <label class="onb-label mono">{t("ENDPOINT")}</label>
              <input
                class="input"
                bind:value={homelabEndpoint}
                placeholder="http://homelab.local:11434"
                style="font-family:var(--font-mono)"
              />
              <button class="btn btn--sm" style="margin-top:10px">
                <Icon name="refresh" size={12} /> {t("Test connection")}
              </button>
            </div>
          {/if}
          <div class="onb-actions">
            <button class="btn btn--ghost" onclick={next}>{t("Skip")}</button>
            <button class="btn btn--primary" onclick={next}>{t("Continue")} <Icon name="arrowR" size={14} /></button>
          </div>
        </div>

      <!-- Step 4: First subject -->
      {:else if step === 4}
        <div class="onb-pane">
          <div class="eyebrow">{t("Last step")}</div>
          <h1 class="read onb-h">{t("Create your first subject.")}</h1>
          <p class="onb-p read">
            {t("A subject holds topics, sources and one living cheatsheet. Name it after a course — you'll add lectures next.")}
          </p>
          <!-- svelte-ignore a11y_label_has_associated_control -->
          <label class="onb-label mono">{t("SUBJECT NAME")}</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="input"
            bind:value={subjName}
            placeholder={t("e.g. Algorithms")}
            autofocus
          />
          <div class="onb-suggest">
            {#each SUGGESTIONS as x}
              <button class="btn btn--sm btn--ghost" onclick={() => (subjName = x)}>{x}</button>
            {/each}
          </div>
          <button class="btn btn--primary onb-cta" onclick={finish}>
            <Icon name="check" size={14} /> {t("Enter Cortex")}
          </button>
        </div>
      {/if}
    </div>

    <!-- Progress bar -->
    <div class="onb-progress">
      <div class="onb-progress-bar" style="width:{((step + 1) / steps.length) * 100}%"></div>
    </div>
  </div>
</div>
