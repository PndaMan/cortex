<script lang="ts">
  // Themed emoji picker — a compact swatch button that opens a popover with
  // category tabs and a grid. Fully styled from theme tokens so it matches the
  // active palette. Pick any emoji for subjects/topics.
  let { value, onPick, size = 30 }: { value: string; onPick: (e: string) => void; size?: number } = $props();

  const CATEGORIES: { id: string; label: string; emojis: string[] }[] = [
    { id: "study", label: "Study", emojis: ["📚","📖","📝","📐","📏","🖊️","✏️","🔖","🏷️","📌","📍","📎","🗂️","📑","📋","📓","📔","🔭","🧮","💡","🧠","🧩","🔍","🎯","🗒️","📰","🗃️","📁"] },
    { id: "science", label: "Science", emojis: ["🔬","🧪","⚗️","🧫","⚛️","🧬","🦠","🌡️","🔋","🔌","🧲","🛰️","🚀","🪐","🌍","🌌","💊","💉","🩺","🦴","🫀","🦋","🐝","🐍"] },
    { id: "arts", label: "Arts", emojis: ["🎨","🖌️","🖼️","🎵","🎼","🎹","🎸","🎻","🥁","🎬","🎭","📷","🎙️","🪕","🎺","💃","🩰","📖"] },
    { id: "symbols", label: "Symbols", emojis: ["◆","●","▲","■","★","✦","✧","❖","✺","❂","⬡","⬢","♦","♠","♣","♥","⚜️","🔰","✚","∞","➕","➗"] },
    { id: "objects", label: "Objects", emojis: ["⚙️","🔧","🔨","🧰","🗜️","🔑","🧭","⏳","⌛","🔒","🔓","💼","📦","🧷","📡","🛡️","⚔️","🏹","🕰️","📅","💰","🏆"] },
    { id: "nature", label: "Nature", emojis: ["🌟","⭐","🔥","💧","❄️","🌊","🌋","⛰️","🏔️","🌙","☀️","⚡","🌈","🍀","🌸","🌻","🍂","🌿","🌳","🌱","🍁","🪐"] },
    { id: "places", label: "Places", emojis: ["🌐","🗺️","🧳","🏛️","🏺","🗿","⚖️","🏰","🗽","⛩️","🕌","🏟️","🏝️","🌍","🌎","🌏"] },
    { id: "faces", label: "Faces", emojis: ["🙂","😀","😎","🤓","🧐","🤔","😴","🥳","😇","🤖","👋","🧑‍🎓","💬","🗣️","👁️","🧑‍🏫"] },
  ];

  let open = $state(false);
  let cat = $state(CATEGORIES[0].id);
  const current = $derived(CATEGORIES.find((c) => c.id === cat) ?? CATEGORIES[0]);

  function choose(e: string) { onPick(e); open = false; }
</script>

<div class="emoji-pick">
  <button
    type="button"
    class="ep-current"
    style:width="{size}px"
    style:height="{size}px"
    title="Pick an emoji"
    onclick={() => (open = !open)}
  >{value || "◆"}</button>

  {#if open}
    <div class="ep-back" role="presentation" onclick={() => (open = false)}></div>
    <div class="ep-pop">
      <div class="ep-tabs">
        {#each CATEGORIES as c}
          <button
            type="button"
            class={"ep-tab" + (c.id === cat ? " on" : "")}
            onclick={() => (cat = c.id)}
          >{c.label}</button>
        {/each}
      </div>
      <div class="ep-grid">
        {#each current.emojis as e}
          <button
            type="button"
            class={"ep-cell" + (e === value ? " on" : "")}
            onclick={() => choose(e)}
          >{e}</button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .emoji-pick { position: relative; display: inline-block; }
  .ep-current {
    display: inline-flex; align-items: center; justify-content: center;
    font-size: 16px; cursor: pointer; border-radius: 8px;
    background: var(--surface-2); border: 1px solid var(--border-strong); color: var(--fg-bright);
    transition: border-color 0.12s ease, background 0.12s ease;
  }
  .ep-current:hover { border-color: var(--accent); background: var(--surface-3); }

  .ep-back { position: fixed; inset: 0; z-index: 60; }
  .ep-pop {
    position: absolute; z-index: 61; top: calc(100% + 6px); left: 0;
    width: 300px; max-width: 78vw; padding: 8px;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: 10px; box-shadow: 0 14px 40px rgba(0,0,0,0.45);
    animation: ep-pop 0.12s ease;
  }
  .ep-tabs { display: flex; flex-wrap: wrap; gap: 3px; margin-bottom: 8px; }
  .ep-tab {
    font: inherit; font-size: 10.5px; cursor: pointer; padding: 3px 7px; border-radius: 999px;
    background: none; border: 1px solid transparent; color: var(--fg-muted);
  }
  .ep-tab:hover { color: var(--fg-bright); background: var(--surface-2); }
  .ep-tab.on { color: var(--accent-fg); background: var(--accent); border-color: var(--accent); }
  .ep-grid {
    display: grid; grid-template-columns: repeat(8, 1fr); gap: 2px;
    max-height: 200px; overflow-y: auto;
  }
  .ep-cell {
    aspect-ratio: 1; display: inline-flex; align-items: center; justify-content: center;
    font-size: 17px; cursor: pointer; border-radius: 6px; background: none; border: 1px solid transparent;
    transition: background 0.1s ease;
  }
  .ep-cell:hover { background: var(--surface-2); }
  .ep-cell.on { border-color: var(--accent); background: var(--surface-3); }
  @keyframes ep-pop { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: none; } }
</style>
