<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  let q = $state("");
  let sel = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  interface CmdItem {
    id: string;
    group: string;
    label: string;
    kind: string;
    icon: string;
    hint?: string;
    run: () => void;
  }

  // Build command list from app state — mirrors the Claude Design command set.
  const commands = $derived<CmdItem[]>([
    ...app.subjects.map((s) => ({
      id: "subj-" + s.id,
      group: "Subjects",
      label: s.name,
      kind: s.code ?? "",
      icon: "diamond",
      run: () => app.openSubject(s.id),
    })),
    { id: "a-record", group: "Actions", label: "Record lecture", kind: "command", icon: "record", hint: "␣ r", run: () => app.setView("recorder") },
    { id: "a-source", group: "Actions", label: "Add source", kind: "command", icon: "plus", hint: "␣ s", run: () => app.setView("add-source") },
    { id: "a-diff", group: "Actions", label: "Review cheatsheet diff", kind: "command", icon: "book", hint: app.pending + " pending", run: () => app.reviewDiff() },
    { id: "a-regen", group: "Actions", label: "Regenerate cheatsheet", kind: "command", icon: "refresh", run: () => app.pushToast({ kind: "info", title: "Regenerating…", body: "Re-synthesizing the cheatsheet from your sources." }) },
    { id: "a-flash", group: "Actions", label: "Study flashcards", kind: "command", icon: "cards", run: () => { app.setView("subject"); app.setTab("materials"); } },
    { id: "a-quiz", group: "Actions", label: "Generate quiz", kind: "command", icon: "check", run: () => { app.setView("subject"); app.setTab("materials"); } },
    { id: "a-web", group: "Actions", label: "Search the web (SearXNG)", kind: "command", icon: "search", hint: "␣ w", run: () => app.setView("websearch") },
    { id: "a-chat", group: "Actions", label: app.chatOpen ? "Hide chat panel" : "Show chat panel", kind: "command", icon: "chat", run: () => app.toggleChat() },
    { id: "a-music", group: "Actions", label: "Study sound…", kind: "command", icon: "music", hint: "m", run: () => (app.musicOpen = true) },
    { id: "a-newsubj", group: "Actions", label: "New subject", kind: "command", icon: "plus", hint: "n", run: () => app.setView("add-subject") },
    { id: "v-dash", group: "Go to", label: "Dashboard", kind: "view", icon: "home", hint: "␣ g", run: () => app.setView("dashboard") },
    { id: "s-settings", group: "Settings", label: "Open settings", kind: "setting", icon: "settings", run: () => app.setView("settings") },
    { id: "s-keys", group: "Settings", label: "API keys & models", kind: "setting", icon: "lock", run: () => app.setView("settings") },
    { id: "s-profile", group: "Settings", label: "Edit your profile", kind: "setting", icon: "diamond", run: () => app.setView("settings") },
    { id: "s-theme", group: "Settings", label: "Cycle Omarchy theme", kind: "setting", icon: "settings", hint: "␣ t", run: () => app.cycleTheme() },
  ]);

  const filtered = $derived(
    commands.filter(
      (c) =>
        !q ||
        c.label.toLowerCase().includes(q.toLowerCase()) ||
        (c.kind || "").toLowerCase().includes(q.toLowerCase())
    )
  );

  // Group by group name
  const groups = $derived(
    filtered.reduce<Record<string, typeof filtered>>((acc, c) => {
      (acc[c.group] ??= []).push(c);
      return acc;
    }, {})
  );

  // Flat list for arrow navigation
  const flat = $derived(filtered);

  $effect(() => {
    if (app.cmdkOpen) {
      q = "";
      sel = 0;
      setTimeout(() => inputEl?.focus(), 20);
    }
  });

  $effect(() => {
    if (sel >= flat.length) sel = Math.max(0, flat.length - 1);
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      e.preventDefault();
      sel = Math.min(flat.length - 1, sel + 1);
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      e.preventDefault();
      sel = Math.max(0, sel - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      runItem(flat[sel]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      app.cmdkOpen = false;
    }
  }

  function runItem(c: CmdItem | undefined) {
    if (!c) return;
    app.cmdkOpen = false;
    c.run?.();
  }
</script>

{#if app.cmdkOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="overlay" onmousedown={() => (app.cmdkOpen = false)}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="cmdk" style:margin-top="12vh" onmousedown={(e) => e.stopPropagation()}>
      <div class="cmdk-input">
        <span class="lead">:</span>
        <input
          bind:this={inputEl}
          bind:value={q}
          oninput={() => { sel = 0; }}
          onkeydown={onKey}
          placeholder="Search actions, subjects, sources…"
        />
        <span class="kbd">esc</span>
      </div>

      <div style:max-height="46vh" style:overflow-y="auto" style:overflow-x="hidden">
        {#if flat.length === 0}
          <div style:padding="22px" style:text-align="center" style:color="var(--fg-faint)" style:font-size="var(--t-sm)">
            No matches
          </div>
        {/if}

        {#each Object.entries(groups) as [groupName, items] (groupName)}
          {@const groupStart = flat.indexOf(items[0])}
          <div class="cmdk-group">
            <div class="gl">{groupName}</div>
            {#each items as c, localIdx (c.id)}
              {@const globalIdx = groupStart + localIdx}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="cmdk-item{globalIdx === sel ? ' sel' : ''}"
                onmouseenter={() => (sel = globalIdx)}
                onclick={() => runItem(c)}
              >
                <span class="ci-ico"><Icon name={c.icon ?? "arrowR"} size={14} /></span>
                {c.label}
                {#if c.hint}
                  <span class="ci-kind">{c.hint}</span>
                {:else}
                  <span class="ci-kind">{c.kind}</span>
                {/if}
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}
