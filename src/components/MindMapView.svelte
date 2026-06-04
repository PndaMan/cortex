<script lang="ts">
  import Icon from "./Icon.svelte";

  // Hierarchical mind map: a central node, main branches, and up to two more
  // nested levels. Rendered as a left-to-right tree with elbow connectors —
  // deterministic, no graph-layout library, prints/exports cleanly.
  interface Node { label?: string; children?: Node[] }
  interface MindData { central?: string; title?: string; branches?: Node[] }

  let { data, onExit }: { data?: MindData; onExit?: () => void } = $props();

  const central = $derived(
    (data?.central ?? data?.title ?? "Mind map").toString()
  );
  const branches = $derived(
    Array.isArray(data?.branches) ? data!.branches!.filter((b) => b && b.label) : []
  );
  // Rotate an accent hue per branch so sub-trees are visually distinct.
  const HUES = ["var(--accent)", "var(--info)", "var(--ok)", "var(--warn)", "var(--mode-select)"];
</script>

<div class="workspace-scroll">
  <div class="mm-page">
    {#if onExit}
      <div class="mm-toolbar">
        <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title="Back to materials">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
        </button>
        <span class="mono faint" style="font-size: var(--t-xs);">Mind map</span>
      </div>
    {/if}

    {#if branches.length === 0}
      <div class="mm-empty">
        <Icon name="link" size={26} color="var(--fg-faint)" />
        <p class="mono faint" style="margin-top: 12px;">No mind-map content.</p>
      </div>
    {:else}
      <div class="mm-canvas">
        <div class="mm-central">{central}</div>
        <div class="mm-branches">
          {#each branches as b, i (i)}
            {@const hue = HUES[i % HUES.length]}
            <div class="mm-branch" style:--hue={hue}>
              <div class="mm-node mm-node--1">{b.label}</div>
              {#if b.children?.length}
                <ul class="mm-children">
                  {#each b.children.filter((c) => c && c.label) as c, j (j)}
                    <li class="mm-child">
                      <span class="mm-node mm-node--2">{c.label}</span>
                      {#if c.children?.length}
                        <ul class="mm-children mm-children--deep">
                          {#each c.children.filter((g) => g && g.label) as g, k (k)}
                            <li class="mm-child">
                              <span class="mm-node mm-node--3">{g.label}</span>
                            </li>
                          {/each}
                        </ul>
                      {/if}
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .mm-page { display: flex; flex-direction: column; min-height: 100%; padding: var(--sp-6) var(--sp-8); gap: var(--sp-5); }
  .mm-toolbar { display: flex; align-items: center; gap: 10px; flex: none; }
  .mm-empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: var(--sp-10); }

  /* Central node centred; branches fan to the right in a responsive grid. */
  .mm-canvas { display: flex; flex-direction: column; align-items: center; gap: var(--sp-6); width: 100%; }
  .mm-central {
    align-self: center;
    font-size: clamp(18px, 2.4vw, 26px);
    font-weight: 750;
    color: var(--bg);
    background: var(--accent);
    padding: 12px 24px;
    border-radius: 999px;
    box-shadow: var(--shadow-2);
    text-align: center;
    max-width: 90%;
  }
  .mm-branches {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--sp-5) var(--sp-6);
    width: 100%;
    align-items: start;
  }
  .mm-branch {
    border: 1px solid color-mix(in oklab, var(--hue) 30%, var(--border));
    border-top: 3px solid var(--hue);
    border-radius: var(--rad-4);
    background: color-mix(in oklab, var(--hue) 5%, var(--surface));
    padding: 14px 14px 12px;
    break-inside: avoid;
  }
  .mm-node { display: inline-block; line-height: 1.3; }
  .mm-node--1 { font-size: 15.5px; font-weight: 700; color: var(--fg-bright); margin-bottom: 8px; }
  .mm-node--2 { font-size: 13.5px; font-weight: 550; color: var(--fg); }
  .mm-node--3 { font-size: 12.5px; color: var(--fg-muted); }

  /* Children as an indented list with elbow connectors drawn via borders. */
  .mm-children { list-style: none; margin: 0; padding: 0 0 0 14px; display: flex; flex-direction: column; gap: 6px; }
  .mm-child { position: relative; padding-left: 14px; }
  .mm-child::before {
    content: ""; position: absolute; left: 0; top: 0; bottom: 50%;
    width: 10px; border-left: 1.5px solid color-mix(in oklab, var(--hue) 45%, var(--border));
    border-bottom: 1.5px solid color-mix(in oklab, var(--hue) 45%, var(--border));
    border-bottom-left-radius: 6px;
  }
  /* extend the vertical guide for all but the last sibling */
  .mm-child:not(:last-child)::after {
    content: ""; position: absolute; left: 0; top: 50%; bottom: -6px;
    width: 1.5px; background: color-mix(in oklab, var(--hue) 45%, var(--border));
  }
  .mm-children--deep { padding-left: 10px; margin-top: 6px; }
</style>
