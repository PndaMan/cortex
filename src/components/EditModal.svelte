<script lang="ts">
  // Rich, fully-themed edit modal — subjects/topics/sources with all their
  // fields. Driven by app.editing; saves through the store's update actions.
  import { app, SUBJECT_COLORS, GLYPHS } from "../lib/store.svelte";
  import Picker from "./Picker.svelte";

  // Local form state, seeded whenever a new target opens.
  let name = $state("");
  let code = $state("");
  let glyph = $state("◆");
  let color = $state(SUBJECT_COLORS[0]);
  let topicId = $state<string>("");
  let tagsText = $state("");
  let firstInput = $state<HTMLInputElement | null>(null);

  $effect(() => {
    const t = app.editing;
    if (!t) return;
    if (t.kind === "subject") {
      name = t.name; code = t.code ?? ""; glyph = t.glyph || "◆"; color = t.color || SUBJECT_COLORS[0];
    } else if (t.kind === "topic") {
      name = t.name;
    } else {
      name = t.name; topicId = t.topicId ?? ""; tagsText = (t.tags ?? []).join(", ");
    }
    queueMicrotask(() => { firstInput?.focus(); firstInput?.select(); });
  });

  const topicOptions = $derived(
    app.editing?.kind === "source"
      ? [...app.editing.topicOptions, { id: "", label: "— no topic —" }]
      : []
  );

  function save() {
    const t = app.editing;
    if (!t || !name.trim()) return;
    if (t.kind === "subject") {
      app.updateSubject(t.id, name.trim(), code.trim() || undefined, glyph.trim() || undefined, color);
    } else if (t.kind === "topic") {
      app.updateTopic(t.id, name.trim());
    } else {
      const tags = tagsText.split(",").map((s) => s.trim()).filter(Boolean);
      app.updateSource(t.id, name.trim(), topicId || null, tags);
    }
    app.closeEdit();
  }

  async function del() {
    const t = app.editing;
    if (!t) return;
    const ok = await app.confirm({ title: `Delete this ${t.kind}?`, danger: true, okLabel: "Delete" });
    if (!ok) return;
    if (t.kind === "subject") app.deleteSubject(t.id);
    else if (t.kind === "topic") app.deleteTopic(t.id);
    else app.deleteSource(t.id);
    app.closeEdit();
  }

  function onKey(e: KeyboardEvent) {
    if (!app.editing) return;
    e.stopPropagation();
    if (e.key === "Escape") { e.preventDefault(); app.closeEdit(); }
    else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); save(); }
  }

  const title = $derived(
    app.editing?.kind === "subject" ? "Edit subject"
      : app.editing?.kind === "topic" ? "Rename topic"
      : "Edit source"
  );
</script>

<svelte:window onkeydown={onKey} />

{#if app.editing}
  {@const t = app.editing}
  <div class="edit-back" role="presentation" onmousedown={() => app.closeEdit()}>
    <div class="edit" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <div class="edit-title">{title}</div>

      <label class="edit-field">
        <span class="edit-lbl">{t.kind === "topic" ? "Topic name" : "Name"}</span>
        <input bind:this={firstInput} bind:value={name} class="input" placeholder="Name" />
      </label>

      {#if t.kind === "subject"}
        <label class="edit-field">
          <span class="edit-lbl">Code</span>
          <input bind:value={code} class="input" placeholder="e.g. PHIL-101" />
        </label>
        <div class="edit-field">
          <span class="edit-lbl">Glyph</span>
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
        <div class="edit-field">
          <span class="edit-lbl">Color</span>
          <div class="edit-colors">
            {#each SUBJECT_COLORS as c}
              <button
                type="button"
                class={"swatch" + (color === c ? " on" : "")}
                style:background={c}
                aria-label={c}
                onclick={() => (color = c)}
              ></button>
            {/each}
            <input type="color" bind:value={color} class="swatch-custom" aria-label="Custom color" />
          </div>
        </div>
      {/if}

      {#if t.kind === "source"}
        <div class="edit-field">
          <span class="edit-lbl">Topic</span>
          <Picker
            value={topicId}
            onChange={(id) => (topicId = id)}
            options={topicOptions}
            placeholder="— no topic —"
          />
        </div>
        <label class="edit-field">
          <span class="edit-lbl">Tags</span>
          <input bind:value={tagsText} class="input" placeholder="comma, separated, tags" />
        </label>
      {/if}

      <div class="edit-actions">
        <button class="btn btn--danger btn--sm" type="button" style="margin-right:auto" onclick={del}>Delete</button>
        <button class="btn btn--ghost btn--sm" type="button" onclick={() => app.closeEdit()}>Cancel</button>
        <button class="btn btn--primary btn--sm" type="button" onclick={save}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .edit-back {
    position: fixed; inset: 0; z-index: 200;
    display: flex; align-items: center; justify-content: center;
    background: color-mix(in oklab, var(--bg) 62%, transparent);
    backdrop-filter: blur(3px);
    animation: e-fade 0.12s ease;
  }
  .edit {
    width: min(480px, calc(100vw - 48px));
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.5);
    padding: 20px;
    animation: e-pop 0.13s ease;
  }
  .edit-title {
    font-family: var(--font-mono); font-size: var(--t-md, 14px);
    font-weight: 600; color: var(--fg-bright); margin-bottom: 14px;
  }
  .edit-field { display: block; margin-top: 12px; }
  .edit-row { display: flex; gap: 12px; align-items: flex-start; }
  .edit-lbl {
    display: block; margin-bottom: 6px;
    font-size: var(--t-2xs, 10.5px); font-weight: 600; letter-spacing: 0.12em;
    text-transform: uppercase; color: var(--fg-faint);
  }
  .edit-field .input { width: 100%; }
  .edit-colors { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .swatch {
    width: 22px; height: 22px; border-radius: 6px; border: 2px solid transparent;
    cursor: pointer; transition: transform 0.1s ease, border-color 0.1s ease;
  }
  .swatch:hover { transform: scale(1.12); }
  .swatch.on { border-color: var(--fg-bright); }
  .swatch-custom {
    width: 26px; height: 26px; padding: 0; border: 1px solid var(--border-strong);
    border-radius: 6px; background: none; cursor: pointer;
  }
  .edit-actions { margin-top: 20px; display: flex; justify-content: flex-end; gap: 8px; }
  @keyframes e-fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes e-pop { from { opacity: 0; transform: translateY(6px) scale(0.98); } to { opacity: 1; transform: none; } }
</style>
