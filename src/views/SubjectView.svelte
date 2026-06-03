<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Source } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Cheatsheet from "./Cheatsheet.svelte";
  import Materials from "./Materials.svelte";
  import ChatPanel from "../components/ChatPanel.svelte";

  const TABS = [
    { id: "cheatsheet", label: "Cheatsheet", icon: "book" },
    { id: "sources", label: "Sources", icon: "doc" },
    { id: "chats", label: "Chats", icon: "chat" },
    { id: "materials", label: "Materials", icon: "grid" },
  ] as const;

  function kindLabel(kind: string): string {
    const map: Record<string, string> = {
      pdf: "PDF", pptx: "PPTX", docx: "DOCX", txt: "TXT",
      md: "MD", web: "WEB", yt: "YT", audio: "AUD", image: "IMG",
    };
    return map[kind] ?? kind.toUpperCase().slice(0, 3);
  }
  const kindBadge = (kind: string) => (kind === "audio" ? "audio" : kind);

  const subj = $derived(app.activeSubject);

  // Load ALL sources for the subject (including ones with no topic, which the
  // subject tree omits) and group them by topic for the Sources tab.
  let srcList = $state<Source[]>([]);
  $effect(() => {
    const id = subj?.id;
    if (!id) { srcList = []; return; }
    api.listSources(id).then((s) => (srcList = s)).catch(() => (srcList = []));
  });
  const groups = $derived.by(() => {
    const m = new Map<string, Source[]>();
    for (const s of srcList) {
      const key = s.topic_id ?? "__none__";
      (m.get(key) ?? m.set(key, []).get(key)!).push(s);
    }
    return [...m.entries()].map(([k, items]) => ({
      key: k,
      name: k === "__none__" ? "Ungrouped" : (subj?.topics.find((t) => t.id === k)?.name ?? "Ungrouped"),
      items,
    }));
  });
</script>

{#if subj}
  <div class="subject-view">
    <!-- Tab bar -->
    <div class="subj-tabs">
      <div class="st-id">
        <span class="subj-glyph sm"><Icon name="diamond" size={13} color="var(--accent)" /></span>
        <div>
          <div class="st-name">{subj.name}</div>
          <div class="st-code mono">{subj.code ?? ""}{subj.topics[0] ? " · " + subj.topics[0].name : ""}</div>
        </div>
      </div>
      <div class="grow"></div>
      {#each TABS as tab (tab.id)}
        <button
          class="subj-tab{app.subjectTab === tab.id ? ' on' : ''}"
          onclick={() => (app.subjectTab = tab.id)}
        >
          <Icon name={tab.icon} size={13} />{tab.label}
        </button>
      {/each}
    </div>

    <!-- Tab body -->
    <div class="subj-body">
      {#if app.subjectTab === "cheatsheet"}
        <Cheatsheet />

      {:else if app.subjectTab === "sources"}
        <div class="workspace-scroll">
          <div class="sources-page">
            <div class="sources-toolbar">
              <span class="label">{srcList.length} {srcList.length === 1 ? "source" : "sources"} · {groups.length} {groups.length === 1 ? "group" : "groups"}</span>
              <div class="grow"></div>
              <button class="btn btn--sm btn--primary" onclick={() => app.setView("add-source")}>
                <Icon name="plus" size={12} /> Add source
              </button>
            </div>

            {#each groups as g (g.key)}
              <div class="src-topic">
                <div class="src-topic-h mono">
                  <Icon name="chevron" size={11} /> {g.name}
                  <span class="faint">· {g.items.length}</span>
                </div>
                <div class="src-grid">
                  {#each g.items as src (src.id)}
                    <button class="source-tile" onclick={() => app.openSource(src)} title="Open source">
                      <div class="stl-top">
                        <span class="badge badge--{kindBadge(src.kind)}">
                          <span class="dot"></span>{kindLabel(src.kind)}
                        </span>
                        <span class="status-pill status-pill--{src.status === 'ready' ? 'ready' : 'draft'}">
                          <span class="dot"></span>
                        </span>
                      </div>
                      <div class="stl-name mono">{src.name}</div>
                      {#if src.meta}
                        <div class="stl-meta mono">{src.meta} · {src.status === "ready" ? "embedded" : src.status === "error" ? "error" : "ingesting…"}</div>
                      {/if}
                      {#if src.tags && src.tags.length > 0}
                        <div class="stl-tags">
                          {#each src.tags as tag (tag)}<span class="src-tag">{tag}</span>{/each}
                        </div>
                      {/if}
                    </button>
                  {/each}
                </div>
              </div>
            {/each}

            {#if srcList.length === 0}
              <div style:text-align="center" style:padding="60px 0" style:color="var(--fg-faint)">
                <Icon name="doc" size={24} />
                <p style:margin-top="12px">No sources yet. Add your first source to get started.</p>
                <button class="btn btn--primary" style:margin-top="16px" onclick={() => app.setView("add-source")}>
                  <Icon name="plus" size={13} /> Add source
                </button>
              </div>
            {/if}
          </div>
        </div>

      {:else if app.subjectTab === "chats"}
        <div class="chats-tab" style:height="100%">
          <ChatPanel />
        </div>

      {:else if app.subjectTab === "materials"}
        <Materials />
      {/if}
    </div>
  </div>
{:else}
  <div class="workspace-scroll">
    <div style:text-align="center" style:padding="80px 32px" style:color="var(--fg-faint)">
      <Icon name="diamond" size={28} />
      <p style:margin-top="16px">Select a subject from the sidebar to get started.</p>
    </div>
  </div>
{/if}
