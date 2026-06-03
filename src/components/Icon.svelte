<script lang="ts">
  let {
    name,
    size = 16,
    color = "currentColor",
    style = "",
  }: { name: string; size?: number; color?: string; style?: string } = $props();

  const sw = 1.6;
</script>

<!-- Inline SVG icon set — paths copied from chrome.jsx Ico -->
<svg
  viewBox="0 0 16 16"
  style="width:{size}px;height:{size}px;display:block;color:{color};{style}"
  aria-hidden="true"
>
  {#if name === "diamond"}
    <path d="M8 2 L14 8 L8 14 L2 8 Z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "chevron"}
    <path d="M6 4 L10 8 L6 12" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "chat"}
    <path d="M2.5 4.5h11v7h-5l-3 2.5v-2.5h-3z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "doc"}
    <path d="M4 2h5l3 3v9H4z M9 2v3h3" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "mic"}
    <g>
      <path d="M8 2.5a1.8 1.8 0 0 1 1.8 1.8v3.4a1.8 1.8 0 0 1-3.6 0V4.3A1.8 1.8 0 0 1 8 2.5z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M4.5 8a3.5 3.5 0 0 0 7 0 M8 11.5V14" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "record"}
    <circle cx="8" cy="8" r="4" fill="currentColor" />
  {:else if name === "cards"}
    <g>
      <path d="M3.5 5h6v8h-6z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M6.5 3.5h6" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M8 5.5l4 .8" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "search"}
    <g>
      <path d="M7 7m-4 0a4 4 0 1 0 8 0a4 4 0 1 0-8 0" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M10 10l3.5 3.5" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "play"}
    <path d="M5 3.5 L12.5 8 L5 12.5 Z" fill="currentColor" />
  {:else if name === "pause"}
    <g>
      <rect x="5" y="3.5" width="2" height="9" rx="0.5" fill="currentColor" />
      <rect x="9" y="3.5" width="2" height="9" rx="0.5" fill="currentColor" />
    </g>
  {:else if name === "music"}
    <g>
      <path d="M6 12.5V4.5l7-1.5v8" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <circle cx="4.4" cy="12.5" r="1.6" fill="currentColor" />
      <circle cx="11.4" cy="11" r="1.6" fill="currentColor" />
    </g>
  {:else if name === "cmd"}
    <path d="M6 6h4v4H6z M6 6a1.4 1.4 0 1 1-1.4-1.4H6 M10 6a1.4 1.4 0 1 0 1.4-1.4H10 M6 10a1.4 1.4 0 1 0-1.4 1.4V10 M10 10a1.4 1.4 0 1 1 1.4 1.4V10" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "bolt"}
    <path d="M9 2 L4 9 H8 L7 14 L12 7 H8 Z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "grid"}
    <g>
      <path d="M3 3h4v4H3z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M9 3h4v4H9z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M3 9h4v4H3z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M9 9h4v4H9z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "refresh"}
    <path d="M13 8a5 5 0 1 1-1.5-3.6 M13 3v2.5h-2.5" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "plus"}
    <path d="M8 3.5v9 M3.5 8h9" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "settings"}
    <g>
      <circle cx="8" cy="8" r="2" />
      <path d="M8 2v1.5 M8 12.5V14 M2 8h1.5 M12.5 8H14 M3.8 3.8l1 1 M11.2 11.2l1 1 M12.2 3.8l-1 1 M4.8 11.2l-1 1" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "home"}
    <path d="M3 8l5-4.5L13 8 M4.5 7v6h7V7" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "check"}
    <path d="M3.5 8.5 L6.5 11.5 L12.5 4.5" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "x"}
    <path d="M4 4l8 8 M12 4l-8 8" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "arrowR"}
    <path d="M3 8h9 M9 5l3 3-3 3" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "book"}
    <path d="M3 3.5h4.5a1.5 1.5 0 0 1 1.5 1.5v8a1.2 1.2 0 0 0-1.2-1.2H3z M13 3.5H8.5A1.5 1.5 0 0 0 7 5v8a1.2 1.2 0 0 1 1.2-1.2H13z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "pencil"}
    <g>
      <path d="M4 12.2l1-3 6.4-6.4 2 2-6.4 6.4z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M9.8 4.6l2 2" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "lock"}
    <g>
      <path d="M4.5 7.5h7v5.5h-7z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M6 7.5V5.5a2 2 0 0 1 4 0v2" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "star"}
    <path d="M8 2.5l1.6 3.5 3.9.4-2.9 2.6.8 3.8L8 11.4 4.6 13.3l.8-3.8L2.5 6.9l3.9-.4z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "waveform"}
    <path d="M2 8h1.5 M4.5 5v6 M6.5 3v10 M8.5 6v4 M10.5 4v8 M12.5 6.5v3 M14 8h.5" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "globe"}
    <g>
      <circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" stroke-width={sw} />
      <path d="M2.5 8h11 M8 2.5c2 2 2 9 0 11 M8 2.5c-2 2-2 9 0 11" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else if name === "external"}
    <path d="M9 3h4v4 M13 3l-6 6 M11 9.5V13H3V5h3.5" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "upload"}
    <path d="M8 11V3 M5 6l3-3 3 3 M3.5 12.5h9" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "camera"}
    <g>
      <path d="M2.5 5.5h2l1-1.5h3l1 1.5h2.5v7h-9.5z" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <circle cx="8" cy="8.5" r="2.2" fill="none" stroke="currentColor" stroke-width={sw} />
    </g>
  {:else if name === "link"}
    <path d="M6.5 9.5l3-3 M6 7l-1.5 1.5a2.1 2.1 0 0 0 3 3L9 10 M10 9l1.5-1.5a2.1 2.1 0 0 0-3-3L7 6" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "reader"}
    <path d="M3 3.5h10v9H3z M5 6h6 M5 8h6 M5 10h4" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
  {:else if name === "calendar"}
    <g>
      <path d="M3 4h10v9H3z M3 6.5h10" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
      <path d="M5.5 2.5v2 M10.5 2.5v2" fill="none" stroke="currentColor" stroke-width={sw} stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {:else}
    <!-- fallback: small square -->
    <rect x="5" y="5" width="6" height="6" fill="none" stroke="currentColor" stroke-width={sw} />
  {/if}
</svg>
