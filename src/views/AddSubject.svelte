<script lang="ts">
  import { app, SUBJECT_COLORS, GLYPHS } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";

  let name = $state("");
  let code = $state("");
  let color = $state(SUBJECT_COLORS[0]);
  let glyph = $state(GLYPHS[0]);
  let topics = $state(["", "", ""]);

  const colors = SUBJECT_COLORS;

  function setTopic(i: number, v: string) {
    topics = topics.map((x, idx) => (idx === i ? v : x));
  }

  const ready = $derived(name.trim().length > 0);

  async function create() {
    if (!ready) return;
    try {
      const subj = await api.createSubject(name.trim(), code.trim() || undefined, glyph, color);
      // Create each non-empty starter topic sequentially. Resilient: a failing
      // topic surfaces a toast but never blocks the rest or the navigation.
      for (const t of topics) {
        const tn = t.trim();
        if (!tn) continue;
        try {
          await api.createTopic(subj.id, tn);
        } catch (te) {
          app.pushToast({ kind: "error", title: "Couldn't add topic", body: `${tn}: ${String(te)}` });
        }
      }
      await app.refresh();
      app.pushToast({ kind: "success", title: "Subject created", body: name.trim() + " is ready." });
      app.setView("dashboard");
    } catch (e) {
      app.pushToast({ kind: "error", title: "Failed to create subject", body: String(e) });
    }
  }
</script>

<div class="workspace-scroll">
  <div class="addpage addpage--subject">
    <div class="addpage-head">
      <button class="btn btn--icon btn--sm btn--ghost" onclick={() => app.setView("dashboard")} title="Back">
        <Icon name="chevron" size={14} style="transform:rotate(180deg)" />
      </button>
      <div>
        <div class="eyebrow">New subject</div>
        <h1 class="addpage-title read">Create a subject</h1>
        <div class="mono faint" style="font-size:var(--t-xs)">holds topics, sources and one living cheatsheet</div>
      </div>
    </div>

    <div class="addsubj-preview">
      <span class="subj-glyph" style="border-color:{color};color:{color};font-size:15px;display:inline-flex;align-items:center;justify-content:center">
        {glyph}
      </span>
      <div>
        <div class="read" style="font-size:var(--r-lg);color:var(--fg-bright)">{name || "Untitled subject"}</div>
        <div class="mono faint" style="font-size:var(--t-2xs)">{code || "no code"} · {topics.filter(t => t.trim()).length} topics</div>
      </div>
    </div>

    <div class="addsubj-form">
      <div class="field">
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="onb-label mono">SUBJECT NAME</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" autofocus bind:value={name} placeholder="e.g. Algorithms" />
      </div>

      <div class="field">
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="onb-label mono">COURSE CODE <span class="faint">optional</span></label>
        <input class="input mono" bind:value={code} placeholder="CS-3490" />
      </div>

      <div class="field">
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="onb-label mono">COLOR</label>
        <div class="color-row">
          {#each colors as c}
            <button
              class={"color-dot" + (color === c ? " on" : "")}
              style="background:{c}"
              onclick={() => (color = c)}
            >
              {#if color === c}
                <Icon name="check" size={12} color="#07140f" />
              {/if}
            </button>
          {/each}
        </div>
      </div>

      <div class="field">
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="onb-label mono">GLYPH</label>
        <div style="display:flex;flex-wrap:wrap;gap:6px">
          {#each GLYPHS as g}
            <button
              type="button"
              style="width:28px;height:28px;border-radius:7px;cursor:pointer;font-size:14px;display:inline-flex;align-items:center;justify-content:center;background:var(--surface-2);border:1px solid {glyph === g ? color : 'var(--border-strong)'};color:{glyph === g ? color : 'var(--fg-muted)'}"
              onclick={() => (glyph = g)}
            >{g}</button>
          {/each}
        </div>
      </div>

      <div class="field">
        <!-- svelte-ignore a11y_label_has_associated_control -->
        <label class="onb-label mono">FIRST TOPICS <span class="faint">optional — add lectures into these</span></label>
        <div class="topic-inputs">
          {#each topics as t, i}
            <input
              class="input"
              value={t}
              oninput={e => setTopic(i, (e.target as HTMLInputElement).value)}
              placeholder={["Recursion", "Dynamic programming", "Graphs"][i] || "Topic"}
            />
          {/each}
        </div>
      </div>
    </div>

    <div class="add-foot">
      <button class="btn btn--ghost" onclick={() => app.setView("dashboard")}>Cancel</button>
      <button class="btn btn--primary" disabled={!ready} onclick={create}>
        <Icon name="check" size={13} /> Create subject
      </button>
    </div>
  </div>
</div>
