<script lang="ts">
  // A thin vertical drag handle for horizontally resizing a panel. Lives on the
  // panel's left/right edge (parent must be position:relative). Reports width via
  // get/set; `sign` is +1 when dragging right widens (left panels) and -1 when
  // dragging left widens (right panels). Calls commit() on release to persist.
  let {
    get,
    set,
    commit,
    side = "right",
    sign = 1,
    min = 180,
    max = 640,
  }: {
    get: () => number;
    set: (v: number) => void;
    commit?: () => void;
    side?: "left" | "right";
    sign?: number;
    min?: number;
    max?: number;
  } = $props();

  let dragging = $state(false);

  function down(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const base = get();
    dragging = true;
    document.body.style.userSelect = "none";
    document.body.style.cursor = "col-resize";
    const move = (ev: PointerEvent) => {
      const v = base + sign * (ev.clientX - startX);
      set(Math.max(min, Math.min(max, v)));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      document.body.style.userSelect = "";
      document.body.style.cursor = "";
      dragging = false;
      commit?.();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }
  // Double-click resets to a sensible default (midpoint of the allowed range).
  function reset() {
    set(Math.round((min + max) / 2));
    commit?.();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="rsz rsz--{side}"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  onpointerdown={down}
  ondblclick={reset}
></div>

<style>
  .rsz {
    position: absolute; top: 0; bottom: 0; width: 7px; z-index: 30;
    cursor: col-resize; touch-action: none;
  }
  .rsz--right { right: -3px; }
  .rsz--left { left: -3px; }
  .rsz::after {
    content: ""; position: absolute; top: 0; bottom: 0; left: 50%; width: 1px;
    transform: translateX(-50%); background: transparent; transition: background 0.12s;
  }
  .rsz:hover::after, .rsz.dragging::after { background: var(--accent); width: 2px; }
</style>
