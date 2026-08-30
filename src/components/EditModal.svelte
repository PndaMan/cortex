<script lang="ts">
  // Rich, fully-themed edit modal — subjects/topics/sources with all their
  // fields. Driven by app.editing; saves through the store's update actions.
  import { app, SUBJECT_COLORS, TOPIC_GLYPHS } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import * as api from "../lib/api";
  import Picker from "./Picker.svelte";
  import EmojiPicker from "./EmojiPicker.svelte";

  // Local form state, seeded whenever a new target opens.
  let name = $state("");
  let code = $state("");
  let glyph = $state("◆");
  let color = $state(SUBJECT_COLORS[0]);
  let topicId = $state<string>("");
  let tagsText = $state("");
  let firstInput = $state<HTMLInputElement | null>(null);
  // Source-branch: which subject this source currently belongs to (or will be moved to).
  let selectedSubjectId = $state<string>("");
  // Source-branch: the subject id the source had when the modal opened (to detect a move).
  let originalSubjectId = $state<string>("");

  $effect(() => {
    const t = app.editing;
    if (!t) return;
    if (t.kind === "subject") {
      name = t.name; code = t.code ?? ""; glyph = t.glyph || "◆"; color = t.color || SUBJECT_COLORS[0];
    } else if (t.kind === "topic") {
      name = t.name; glyph = t.glyph || TOPIC_GLYPHS[0]; tagsText = (t.tags ?? []).join(", ");
    } else {
      name = t.name; topicId = t.topicId ?? ""; tagsText = (t.tags ?? []).join(", ");
      // The source's current subject is carried on the target (no app.subjects
      // scan here, so a background refresh can't reset the open form).
      selectedSubjectId = t.subjectId;
      originalSubjectId = t.subjectId;
    }
    queueMicrotask(() => { firstInput?.focus(); firstInput?.select(); });
  });

  const subjectOptions = $derived(
    app.subjects.map((s) => ({ id: s.id, label: s.name }))
  );

  // Topic options are derived from the currently selected subject (not the
  // static topicOptions baked into EditTarget, which only reflects the active subject).
  const topicOptions = $derived(
    app.editing?.kind === "source"
      ? [
          ...(app.subjects.find((s) => s.id === selectedSubjectId)?.topics ?? []).map((tp) => ({
            id: tp.id,
            label: tp.name,
          })),
          { id: "", label: t("— no topic —") },
        ]
      : []
  );

  async function save() {
    const ed = app.editing;
    if (!ed || !name.trim()) return;
    if (ed.kind === "subject") {
      app.updateSubject(ed.id, name.trim(), code.trim() || undefined, glyph.trim() || undefined, color);
      app.closeEdit();
    } else if (ed.kind === "topic") {
      const tags = tagsText.split(",").map((s) => s.trim()).filter(Boolean);
      app.updateTopic(ed.id, name.trim(), ed.subjectId, glyph.trim() || undefined, tags);
      app.closeEdit();
    } else {
      const tags = tagsText.split(",").map((s) => s.trim()).filter(Boolean);
      if (selectedSubjectId !== originalSubjectId) {
        // Subject changed → move the source to the new subject/topic.
        try {
          await api.moveSource(ed.id, selectedSubjectId, topicId || null);
          await app.refresh();
          app.pushToast({ kind: "success", title: t("Source moved") });
          app.closeEdit();
        } catch (e) {
          app.pushToast({ kind: "error", title: t("Move failed"), body: String(e) });
        }
      } else {
        // Same subject → use normal update path (name/topic/tags).
        app.updateSource(ed.id, name.trim(), topicId || null, tags);
        app.closeEdit();
      }
    }
  }

  async function del() {
    const ed = app.editing;
    if (!ed) return;
    const ok = await app.confirm({ title: t("Delete this {kind}?", { kind: ed.kind }), danger: true, okLabel: t("Delete") });
    if (!ok) return;
    if (ed.kind === "subject") app.deleteSubject(ed.id);
    else if (ed.kind === "topic") app.deleteTopic(ed.id, ed.subjectId);
    else app.deleteSource(ed.id);
    app.closeEdit();
  }

  async function archive() {
    const ed = app.editing;
    if (!ed || ed.kind !== "subject") return;
    const ok = await app.confirm({
      title: t("Archive \"{name}\"?", { name: ed.name }),
      body: t("It's hidden everywhere but its data is kept. Restore it any time from Settings → Data."),
      okLabel: t("Archive"),
    });
    if (!ok) return;
    await app.setSubjectArchived(ed.id, true);
    app.closeEdit();
  }

  function onKey(e: KeyboardEvent) {
    if (!app.editing) return;
    e.stopPropagation();
    if (e.key === "Escape") { e.preventDefault(); app.closeEdit(); }
    else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); save(); }
  }

  const title = $derived(
    app.editing?.kind === "subject" ? t("Edit subject")
      : app.editing?.kind === "topic" ? t("Rename topic")
      : t("Edit source")
  );
</script>

<svelte:window onkeydown={onKey} />

{#if app.editing}
  {@const ed = app.editing}
  <div class="edit-back" role="presentation" onmousedown={() => app.closeEdit()}>
    <div class="edit" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <div class="edit-title">{title}</div>

      <label class="edit-field">
        <span class="edit-lbl">{ed.kind === "topic" ? t("Topic name") : t("Name")}</span>
        <input bind:this={firstInput} bind:value={name} class="input" placeholder={t("Name")} />
      </label>

      {#if ed.kind === "topic"}
        <div class="edit-field">
          <span class="edit-lbl">{t("Icon")}</span>
          <EmojiPicker value={glyph} onPick={(e) => (glyph = e)} />
        </div>
        <label class="edit-field">
          <span class="edit-lbl">{t("Tags")} <span class="faint">{t("e.g. A2 · chat & deadlines group by tag")}</span></span>
          <input bind:value={tagsText} class="input" placeholder={t("comma, separated, tags")} />
        </label>
      {/if}

      {#if ed.kind === "subject"}
        <label class="edit-field">
          <span class="edit-lbl">{t("Code")}</span>
          <input bind:value={code} class="input" placeholder={t("e.g. PHIL-101")} />
        </label>
        <div class="edit-field">
          <span class="edit-lbl">{t("Glyph")}</span>
          <EmojiPicker value={glyph} onPick={(e) => (glyph = e)} />
        </div>
        <div class="edit-field">
          <span class="edit-lbl">{t("Color")}</span>
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
            <input type="color" bind:value={color} class="swatch-custom" aria-label={t("Custom color")} />
          </div>
        </div>
      {/if}

      {#if ed.kind === "source"}
        <div class="edit-field">
          <span class="edit-lbl">{t("Subject")}</span>
          <Picker
            value={selectedSubjectId}
            onChange={(id) => { selectedSubjectId = id; topicId = ""; }}
            options={subjectOptions}
            placeholder={t("— select subject —")}
          />
        </div>
        <div class="edit-field">
          <span class="edit-lbl">{t("Topic")}</span>
          <Picker
            value={topicId}
            onChange={(id) => (topicId = id)}
            options={topicOptions}
            placeholder={t("— no topic —")}
          />
        </div>
        <label class="edit-field">
          <span class="edit-lbl">{t("Tags")}</span>
          <input bind:value={tagsText} class="input" placeholder={t("comma, separated, tags")} />
        </label>
      {/if}

      <div class="edit-actions">
        <button class="btn btn--danger btn--sm" type="button" style="margin-right:auto" onclick={del}>{t("Delete")}</button>
        {#if ed.kind === "subject"}
          <button class="btn btn--ghost btn--sm" type="button" onclick={archive}>{t("Archive")}</button>
        {/if}
        <button class="btn btn--ghost btn--sm" type="button" onclick={() => app.closeEdit()}>{t("Cancel")}</button>
        <button class="btn btn--primary btn--sm" type="button" onclick={save}>{t("Save")}</button>
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
