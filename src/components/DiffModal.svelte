<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";
  import { diff } from "../lib/mock";
  import { flip } from "svelte/animate";

  // Local UI state — all Svelte 5 runes
  let view = $state<"inline" | "split">("inline");
  let resolving = $state<Record<string, "accept" | "reject">>({});
  let focus = $state(0);
  let bodyEl = $state<HTMLElement | null>(null);
  // Live list — resolved sections are removed so the rest snap up (flip animation).
  let sections = $state(diff.sections.map((s) => ({ ...s })));

  // Reset state whenever modal opens
  $effect(() => {
    if (app.diffOpen) {
      resolving = {};
      focus = 0;
      sections = diff.sections.map((s) => ({ ...s }));
    }
  });

  // Keyboard handler — capture phase, installed only while open
  $effect(() => {
    if (!app.diffOpen) return;

    window.__cortexModalOpen = true;

    function onKey(e: KeyboardEvent) {
      const list = sections;
      if (e.key === "Escape") {
        e.preventDefault();
        close();
        return;
      }
      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        focus = Math.min(list.length - 1, focus + 1);
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        focus = Math.max(0, focus - 1);
      } else if (e.key === "a") {
        e.preventDefault();
        const s = list[focus];
        if (s && !resolving[s.id]) resolve(s.id, "accept");
      } else if (e.key === "x") {
        e.preventDefault();
        const s = list[focus];
        if (s && !resolving[s.id]) resolve(s.id, "reject");
      } else if (e.key === "A") {
        e.preventDefault();
        acceptAll();
      } else if (e.key === "s") {
        e.preventDefault();
        view = view === "inline" ? "split" : "inline";
      }
    }

    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.__cortexModalOpen = false;
    };
  });

  // Scroll focused section into view
  $effect(() => {
    if (!app.diffOpen || !bodyEl) return;
    const node = bodyEl.querySelector<HTMLElement>(".diff-section.is-focus");
    if (node) {
      const b = bodyEl.getBoundingClientRect();
      const n = node.getBoundingClientRect();
      if (n.top < b.top + 8 || n.bottom > b.bottom - 8) {
        bodyEl.scrollTo({ top: bodyEl.scrollTop + (n.top - b.top) - 16, behavior: "smooth" });
      }
    }
  });

  function close() {
    app.diffOpen = false;
  }

  function resolve(secId: string, kind: "accept" | "reject") {
    if (resolving[secId]) return;
    resolving = { ...resolving, [secId]: kind };
    // After the resolve flash, drop the section so the remaining ones snap up.
    setTimeout(() => {
      sections = sections.filter((s) => s.id !== secId);
      const { [secId]: _, ...rest } = resolving;
      resolving = rest;
      if (focus >= sections.length) focus = Math.max(0, sections.length - 1);
      if (sections.length === 0) app.mergeDiff();
    }, 420);
  }

  function acceptAll() {
    const next: Record<string, "accept" | "reject"> = {};
    sections.forEach((s) => (next[s.id] = "accept"));
    resolving = next;
    setTimeout(() => app.mergeDiff(), 460);
  }

  const remaining = $derived(sections.filter((s) => !resolving[s.id]));
</script>

{#if app.diffOpen}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay diff-overlay"
    onmousedown={close}
    role="dialog"
    aria-modal="true"
    aria-label="Approve diff"
  >
    <!-- Modal panel — stop propagation so backdrop click doesn't close when clicking inside -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="diff-modal"
      onmousedown={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <header class="diff-head">
        <div>
          <div class="eyebrow">Approve to merge</div>
          <div class="diff-title mono">
            Draft from
            <span class="badge badge--web" style="margin-left: 4px;">
              <span class="dot"></span>{diff.source}
            </span>
          </div>
        </div>
        <div class="diff-head-tools">
          <div class="seg-toggle" title="Toggle with s">
            <button class={view === "inline" ? "on" : ""} onclick={() => (view = "inline")}>Inline</button>
            <button class={view === "split" ? "on" : ""} onclick={() => (view = "split")}>Side-by-side</button>
          </div>
          <button class="btn btn--icon btn--sm btn--ghost" onclick={close}>
            <Icon name="x" size={12} />
          </button>
        </div>
      </header>

      <!-- Legend -->
      <div class="diff-legend mono">
        <span><i class="lg add"></i> added</span>
        <span><i class="lg del"></i> removed</span>
        <span class="diff-keys">
          <span class="kbd">j</span><span class="kbd">k</span> move ·
          <span class="kbd">a</span> accept ·
          <span class="kbd">x</span> reject ·
          <span class="kbd">A</span> all ·
          <span class="kbd">s</span> view
        </span>
      </div>

      <!-- Diff body -->
      <div class="diff-body" bind:this={bodyEl}>
        {#each sections as sec, idx (sec.id)}
          {@const r = resolving[sec.id]}
          {@const isFocus = idx === focus && !r}
          <div
            class="diff-section{r ? ' resolving-' + r : ''}{isFocus ? ' is-focus' : ''}"
            animate:flip={{ duration: 240 }}
            onmouseenter={() => (focus = idx)}
            role="group"
            aria-label={sec.title}
          >
            <div class="diff-sec-head">
              <h3>{sec.title}</h3>
              <div class="diff-stats mono">
                <span class="st-add">+{sec.changes.filter((c) => c.type === "add").length}</span>
                <span class="st-del">−{sec.changes.filter((c) => c.type === "del").length}</span>
              </div>
              <div class="grow"></div>

              {#if !r}
                <button
                  class="btn btn--sm btn--ghost"
                  onclick={() => resolve(sec.id, "reject")}
                  title="Reject (x)"
                >
                  <span class="kbd">x</span> Reject
                </button>
                <button
                  class="btn btn--sm btn--primary"
                  onclick={() => resolve(sec.id, "accept")}
                  title="Accept section (a)"
                >
                  <span class="kbd" style="border-color: currentColor;">a</span> Accept section
                </button>
              {:else if r === "accept"}
                <span class="resolved-tag ok"><Icon name="check" size={13} /> merged</span>
              {:else if r === "reject"}
                <span class="resolved-tag err"><Icon name="x" size={12} /> rejected</span>
              {/if}
            </div>

            {#if view === "inline"}
              <div class="diff-inline">
                {#each sec.changes as c, i (i)}
                  <div class="diff-line {c.type}">
                    <span class="gutter">{c.type === "add" ? "+" : c.type === "del" ? "−" : ""}</span>
                    <span class="txt read">{c.text}</span>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="diff-split">
                <div class="ds-col">
                  <div class="ds-col-l mono">Current</div>
                  {#each sec.changes.filter((c) => c.type !== "add") as c, i (i)}
                    <div class="diff-line {c.type === 'del' ? 'del' : ''}">
                      <span class="txt read">{c.text}</span>
                    </div>
                  {/each}
                </div>
                <div class="ds-col">
                  <div class="ds-col-l mono">Proposed</div>
                  {#each sec.changes.filter((c) => c.type !== "del") as c, i (i)}
                    <div class="diff-line {c.type === 'add' ? 'add' : ''}">
                      <span class="txt read">{c.text}</span>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/each}
      </div>

      <!-- Footer -->
      <footer class="diff-foot">
        <div class="mono faint">
          {remaining.length} section{remaining.length !== 1 ? "s" : ""} awaiting decision
        </div>
        <div class="grow"></div>
        <button class="btn btn--sm btn--ghost" onclick={close}>Later</button>
        <button class="btn btn--sm btn--danger" onclick={close}>Reject all</button>
        <button class="btn btn--sm btn--primary" onclick={acceptAll}>
          <span class="kbd" style="border-color: currentColor;">A</span> Approve all &amp; merge
        </button>
      </footer>
    </div>
  </div>
{/if}
