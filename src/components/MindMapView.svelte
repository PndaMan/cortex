<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import Icon from "./Icon.svelte";

  // Hierarchical mind map: a central node, main branches, and up to two more
  // nested levels. Rendered as a left-to-right tree with elbow connectors —
  // deterministic, no graph-layout library, prints/exports cleanly. Nodes with
  // children are collapsible: clicking the header (or its twisty) hides/shows
  // the subtree below it.
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

  // Only keep children that actually carry a label — used everywhere we recurse.
  const kids = (n?: Node): Node[] =>
    Array.isArray(n?.children) ? n!.children!.filter((c) => c && c.label) : [];

  // Collapse state is keyed by node path ("0", "0.2", "0.2.1"). A key present in
  // the set means that node is COLLAPSED; default (empty set) = everything open,
  // so the central node and all branches are visible on first render.
  const collapsed = new SvelteSet<string>();
  const isOpen = (key: string) => !collapsed.has(key);
  function toggle(key: string) {
    if (collapsed.has(key)) collapsed.delete(key);
    else collapsed.add(key);
  }

  // Walk the tree collecting the path keys of every node that has children;
  // these are the only nodes that can be collapsed.
  function expandablePaths(nodes: Node[], prefix = ""): string[] {
    const out: string[] = [];
    nodes.forEach((n, i) => {
      const key = prefix ? `${prefix}.${i}` : `${i}`;
      const cs = kids(n);
      if (cs.length) {
        out.push(key);
        out.push(...expandablePaths(cs, key));
      }
    });
    return out;
  }
  const expandAll = () => collapsed.clear();
  function collapseAll() {
    collapsed.clear();
    for (const key of expandablePaths(branches)) collapsed.add(key);
  }
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
      <div class="mm-controls">
        <button class="btn btn--sm btn--ghost" onclick={expandAll}>Expand all</button>
        <button class="btn btn--sm btn--ghost" onclick={collapseAll}>Collapse all</button>
      </div>

      <div class="mm-canvas">
        <div class="mm-central">{central}</div>
        <div class="mm-branches">
          {#each branches as b, i (i)}
            {@const hue = HUES[i % HUES.length]}
            {@const bKey = `${i}`}
            {@const bKids = kids(b)}
            {@const bOpen = isOpen(bKey)}
            <div class="mm-branch" style:--hue={hue}>
              {#if bKids.length}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <div
                  class="mm-node mm-node--1 mm-head"
                  role="button"
                  tabindex="0"
                  aria-expanded={bOpen}
                  onclick={() => toggle(bKey)}
                  onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), toggle(bKey))}
                >
                  <span class="mm-tw{bOpen ? ' open' : ''}"><Icon name="chevron" size={12} /></span>
                  <span class="mm-label">{b.label}</span>
                  <span class="mm-count mono">{bKids.length}</span>
                </div>
                {#if bOpen}
                  <ul class="mm-children">
                    {#each bKids as c, j (j)}
                      {@const cKey = `${bKey}.${j}`}
                      {@const cKids = kids(c)}
                      {@const cOpen = isOpen(cKey)}
                      <li class="mm-child">
                        {#if cKids.length}
                          <!-- svelte-ignore a11y_click_events_have_key_events -->
                          <span
                            class="mm-node mm-node--2 mm-head"
                            role="button"
                            tabindex="0"
                            aria-expanded={cOpen}
                            onclick={() => toggle(cKey)}
                            onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), toggle(cKey))}
                          >
                            <span class="mm-tw{cOpen ? ' open' : ''}"><Icon name="chevron" size={12} /></span>
                            <span class="mm-label">{c.label}</span>
                            <span class="mm-count mono">{cKids.length}</span>
                          </span>
                          {#if cOpen}
                            <ul class="mm-children mm-children--deep">
                              {#each cKids as g, k (k)}
                                <li class="mm-child">
                                  <span class="mm-node mm-node--3">{g.label}</span>
                                </li>
                              {/each}
                            </ul>
                          {/if}
                        {:else}
                          <span class="mm-node mm-node--2">{c.label}</span>
                        {/if}
                      </li>
                    {/each}
                  </ul>
                {/if}
              {:else}
                <div class="mm-node mm-node--1">{b.label}</div>
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
  .mm-controls { display: flex; align-items: center; gap: var(--sp-2); flex: none; }
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

  /* Collapsible header: twisty + label + child-count hint, clickable. */
  .mm-head {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    border-radius: var(--rad-1);
    user-select: none;
  }
  .mm-head:hover .mm-label { color: var(--fg-bright); }
  .mm-head:focus-visible { outline: 1.5px solid var(--hue); outline-offset: 2px; }
  .mm-head .mm-label { flex: 1; min-width: 0; }
  .mm-tw {
    flex: none;
    display: inline-flex;
    align-items: center;
    color: var(--hue);
    transition: transform var(--dur-fast) var(--ease);
  }
  .mm-tw.open { transform: rotate(90deg); }
  .mm-count {
    flex: none;
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    background: color-mix(in oklab, var(--hue) 12%, var(--surface-2));
    border-radius: var(--rad-pill);
    padding: 1px 7px;
    line-height: 1.4;
  }

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

  @media (max-width: 600px) {
    .mm-page { padding: var(--sp-5) var(--sp-4); }
    .mm-branches { grid-template-columns: 1fr; gap: var(--sp-4); }
  }
</style>
