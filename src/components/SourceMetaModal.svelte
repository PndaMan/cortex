<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  const SUGGESTED_TAGS = ["lecture", "exam-relevant", "needs-review", "key-concept", "worked-example", "reference"];

  // Local editable state, synced from app.metaModal when it opens
  let title = $state("");
  let tags = $state<string[]>([]);
  let tagDraft = $state("");
  // Inline picker state for subject/topic (SourceMetaModal uses mock subjects in the prototype;
  // in Svelte we just edit the modal payload if subjects were needed, but since app.metaModal
  // carries full context we track the displayed topic field)
  let topicDraft = $state("");

  // Sync local state from app.metaModal whenever it becomes non-null
  $effect(() => {
    const m = app.metaModal;
    if (m) {
      title = m.title ?? "";
      tags = Array.isArray(m.tags) ? [...m.tags] : [];
      tagDraft = "";
      topicDraft = m.topic ?? "";
    }
  });

  const isRecord = $derived(app.metaModal?.mode === "record");
  const type = $derived(app.metaModal?.type ?? "audio");
  const typeLabel = $derived(
    ({ pdf: "PDF", pptx: "PPTX", docx: "DOCX", web: "WEB", yt: "YT", audio: "AUDIO", image: "IMAGE" } as Record<string, string>)[type]
    ?? type.toUpperCase()
  );

  function addTag(raw?: string) {
    const t = (raw ?? tagDraft).trim().toLowerCase().replace(/\s+/g, "-");
    if (t && !tags.includes(t)) tags = [...tags, t];
    tagDraft = "";
  }

  function removeTag(t: string) {
    tags = tags.filter(x => x !== t);
  }

  function handleTagKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); addTag(); }
    else if (e.key === "Backspace" && !tagDraft && tags.length) removeTag(tags[tags.length - 1]);
  }

  function save() {
    app.pushToast({
      kind: "success",
      title: isRecord ? "Recording saved" : "Source updated",
      body: title.trim() || (isRecord ? "Untitled recording" : "Untitled source"),
    });
    app.metaModal = null;
  }

  function close() {
    app.metaModal = null;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }
</script>

<svelte:window onkeydown={onKeyDown} />

{#if app.metaModal !== null}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" onmousedown={close}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="meta-modal" onmousedown={e => e.stopPropagation()}>
      <header class="meta-head">
        <div>
          <div class="eyebrow">{isRecord ? "Save recording" : "Edit source"}</div>
          <div class="meta-title-h mono">{isRecord ? "New lecture recording" : "Source details"}</div>
        </div>
        <button class="btn btn--icon btn--sm btn--ghost" onclick={close}>
          <Icon name="x" size={12} />
        </button>
      </header>

      <div class="meta-body">
        {#if isRecord}
          <div class="meta-rec-note mono">
            <Icon name="check" size={13} color="var(--ok)" />
            Recorded {app.metaModal?.meta ?? ""} · transcribed. Confirm where it lives before saving.
          </div>
        {/if}

        <!-- Title field -->
        <div class="field">
          <label class="onb-label mono">SOURCE TITLE</label>
          <div class="meta-title-row">
            <span class={"badge badge--" + (type === "audio" ? "audio" : type)}>
              <span class="dot"></span>{typeLabel}
            </span>
            <input
              class="input"
              autofocus
              bind:value={title}
              placeholder="Give this source a clear name…"
            />
          </div>
        </div>

        <!-- Topic field (simple input, no full picker needed without backend subjects here) -->
        <div class="meta-grid">
          <div class="field">
            <label class="onb-label mono">SUBJECT</label>
            <div class="picker">
              <button type="button" class="picker-btn">
                <Icon name="diamond" size={12} color="var(--fg-faint)" />
                <span class="picker-val">{app.activeSubject?.name ?? "—"}</span>
                <Icon name="chevron" size={11} style="transform:rotate(90deg);color:var(--fg-faint)" />
              </button>
            </div>
          </div>
          <div class="field">
            <label class="onb-label mono">TOPIC</label>
            <div class="picker">
              <button type="button" class="picker-btn">
                <Icon name="chevron" size={12} color="var(--fg-faint)" />
                <span class="picker-val {topicDraft ? '' : 'ph'}">{topicDraft || "Choose topic"}</span>
                <Icon name="chevron" size={11} style="transform:rotate(90deg);color:var(--fg-faint)" />
              </button>
            </div>
          </div>
        </div>

        <!-- Tags -->
        <div class="field">
          <label class="onb-label mono">TAGS</label>
          <div class="tag-editor">
            {#each tags as t}
              <span class="src-tag editable">
                {t}
                <button class="tag-x" onclick={() => removeTag(t)}>
                  <Icon name="x" size={9} />
                </button>
              </span>
            {/each}
            <input
              class="tag-input mono"
              bind:value={tagDraft}
              placeholder={tags.length ? "add…" : "type a tag, press Enter…"}
              onkeydown={handleTagKeyDown}
            />
          </div>
          <div class="tag-suggest">
            {#each SUGGESTED_TAGS.filter(s => !tags.includes(s)) as s}
              <button class="tag-chip-add" onclick={() => addTag(s)}>
                <Icon name="plus" size={9} /> {s}
              </button>
            {/each}
          </div>
        </div>
      </div>

      <footer class="meta-foot">
        <button class="btn btn--ghost" onclick={close}>Cancel</button>
        <button class="btn btn--primary" onclick={save}>
          <Icon name="check" size={13} /> {isRecord ? "Save source" : "Save changes"}
        </button>
      </footer>
    </div>
  </div>
{/if}
