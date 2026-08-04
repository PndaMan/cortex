<script lang="ts">
  // Subject detail panel (modal). Opened by clicking the subject header in
  // SubjectView. Holds the subject's About info, the Moodle link + synced
  // grades/deadlines/announcements, and the module framework — whose original file
  // opens in-app as a PDF, like a source.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "./Icon.svelte";
  import Picker from "./Picker.svelte";
  import TopicRow from "./TopicRow.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { isMobile } from "../lib/platform";

  const subj = $derived(app.activeSubject);

  // ── Moodle state ──────────────────────────────────────────────
  let mdStatus = $state<api.MoodleStatus>({ configured: false, user_id: 0, last_sync: 0 });
  let mdData = $state<api.MoodleData>({ courses: [], grades: [], deadlines: [], announcements: [] });
  let syncing = $state(false);
  let linking = $state(false);
  // Moodle base URL (setting) → used to deep-link "Open in Moodle".
  let moodleUrl = $state("");

  async function loadMoodle() {
    try {
      mdStatus = await api.moodleStatus();
      mdData = mdStatus.configured
        ? await api.moodleData()
        : { courses: [], grades: [], deadlines: [], announcements: [] };
      moodleUrl = (await api.getSetting("moodle_url")) ?? "";
    } catch {
      /* leave defaults — surfaced as "not connected" */
    }
  }

  const linkedCourseId = $derived(subj?.moodle_course_id ?? null);
  const linkedCourse = $derived(
    linkedCourseId ? mdData.courses.find((c) => c.id === linkedCourseId) ?? null : null
  );
  // Moodle course permalink: {base}/course/view.php?id={courseId}.
  const courseUrl = $derived(
    moodleUrl && linkedCourseId
      ? `${moodleUrl.replace(/\/+$/, "")}/course/view.php?id=${linkedCourseId}`
      : null
  );
  // On a phone, deep-link into the Moodle mobile app (custom scheme) instead of a
  // browser tab; if that fails (Moodle app not installed / scheme unhandled),
  // fall back to the web URL in the browser. Desktop goes straight to the web.
  function openCourse() {
    if (!courseUrl) return;
    const web = courseUrl;
    if (isMobile) {
      api.openExternal(`moodlemobile://link=${encodeURIComponent(web)}`).catch(() => {
        void api.openExternal(web).catch(() => {});
      });
    } else {
      void api.openExternal(web).catch(() => {});
    }
  }
  const courseGrades = $derived(
    linkedCourseId ? mdData.grades.filter((g) => g.course_id === linkedCourseId) : []
  );
  const courseDeadlines = $derived(
    linkedCourseId
      ? mdData.deadlines.filter((d) => d.course_id === linkedCourseId).sort((a, b) => a.due_at - b.due_at)
      : []
  );
  const courseAnnouncements = $derived(
    linkedCourseId
      ? mdData.announcements.filter((a) => a.course_id === linkedCourseId).sort((a, b) => b.posted_at - a.posted_at)
      : []
  );
  const courseOptions = $derived(
    mdData.courses.map((c) => ({ id: c.id, label: c.fullname || c.shortname || c.id }))
  );

  async function linkCourse(courseId: string) {
    if (!subj || !courseId) return;
    linking = true;
    try {
      await api.moodleLinkSubject(subj.id, courseId);
      await app.refresh();
      app.pushToast({ kind: "success", title: "Linked to Moodle course" });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Link failed", body: String(e) });
    } finally {
      linking = false;
    }
  }
  async function unlinkCourse() {
    if (!subj) return;
    try {
      await api.moodleLinkSubject(subj.id, null);
      await app.refresh();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Unlink failed", body: String(e) });
    }
  }
  async function autoMatch() {
    linking = true;
    try {
      const n = await api.moodleAutolink();
      await app.refresh();
      app.pushToast({
        kind: n > 0 ? "success" : "info",
        title: n > 0 ? `Matched ${n} subject${n === 1 ? "" : "s"}` : "No new matches",
        body: n > 0 ? undefined : "Pick the course manually below.",
      });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Auto-match failed", body: String(e) });
    } finally {
      linking = false;
    }
  }
  async function syncNow() {
    syncing = true;
    try {
      const s = await api.moodleSync();
      await loadMoodle();
      await app.loadMoodleData();   // refresh the app-wide feed + badge
      app.notifyEventsChanged();    // deadlines mirrored into the calendar
      app.pushToast({
        kind: "success",
        title: "Moodle synced",
        body: `${s.grades} grades · ${s.deadlines} deadlines · ${s.announcements} announcements`,
      });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Sync failed", body: String(e) });
    } finally {
      syncing = false;
    }
  }

  // ── Module framework ──────────────────────────────────────────
  let framework = $state<api.FrameworkMeta | null>(null);
  let fwBusy = $state(false);
  // Document viewer overlay state.
  let fwViewing = $state(false);
  let fwText = $state<string | null>(null);
  const fwSrc = $derived(framework?.file_path ? convertFileSrc(framework.file_path) : "");

  // Open an announcement/deadline in the shared themed detail reader.
  const courseLabel = $derived(linkedCourse?.fullname || linkedCourse?.shortname || "");
  function openAnnouncement(a: api.MoodleAnnouncement) {
    app.openDetail({
      id: a.id, kind: "announcement", title: a.subject, course: courseLabel,
      ts: a.posted_at * 1000, url: a.url, message: a.message, subjectId: subj?.id ?? null,
    });
  }
  function openDeadline(d: api.MoodleDeadline) {
    app.openDetail({
      id: d.id, kind: d.kind === "exam" ? "exam" : "deadline", title: d.name, course: courseLabel,
      ts: d.due_at * 1000, url: d.url, message: "", subjectId: subj?.id ?? null,
    });
  }

  async function loadFramework() {
    const id = subj?.id;
    if (!id) { framework = null; return; }
    framework = await api.getSubjectFramework(id).catch(() => null);
  }
  async function uploadFramework() {
    if (!subj) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Documents", extensions: ["pdf", "epub", "docx", "pptx", "doc", "ppt", "txt", "md", "png", "jpg", "jpeg"] }],
      });
      const path = typeof picked === "string" ? picked : picked?.[0] ?? null;
      if (!path) return;
      fwBusy = true;
      framework = await api.setSubjectFramework(subj.id, path);
      app.pushToast({ kind: "success", title: "Module framework saved", body: framework.filename });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Could not read framework", body: String(e) });
    } finally {
      fwBusy = false;
    }
  }
  async function removeFramework() {
    if (!subj) return;
    if (!(await app.confirm({ title: "Remove module framework?", okLabel: "Remove", danger: true }))) return;
    try {
      await api.clearSubjectFramework(subj.id);
      framework = null;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Remove failed", body: String(e) });
    }
  }
  async function viewFramework() {
    if (!subj || !framework) return;
    fwText = null;
    if (framework.view_kind === "text" || !framework.file_path) {
      fwText = (await api.getSubjectFrameworkText(subj.id).catch(() => null)) ?? "";
    }
    fwViewing = true;
  }

  async function addTopic() {
    const name = await app.prompt({ title: "Add topic", label: "Topic name", placeholder: "e.g. Determinism" });
    if (name) await app.createTopic(name);
  }

  // Calendar match keywords (no AI): comma-separated terms that appear in this
  // subject's timetable events, so lectures auto-file to it.
  let aliasInput = $state("");
  $effect(() => { aliasInput = subj?.calendar_aliases ?? ""; });
  async function saveAliases() {
    if (!subj || aliasInput === (subj.calendar_aliases ?? "")) return;
    try {
      const n = await api.setSubjectAliases(subj.id, aliasInput);
      await app.refresh();
      app.notifyEventsChanged();
      if (n > 0) app.pushToast({ kind: "success", title: `Filed ${n} calendar event${n === 1 ? "" : "s"}` });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't save keywords", body: String(e) });
    }
  }

  function editSubject() {
    if (!subj) return;
    app.openEdit({
      kind: "subject",
      id: subj.id,
      name: subj.name,
      code: subj.code ?? "",
      glyph: subj.glyph,
      color: app.subjectColor(subj),
    });
  }

  // Load Moodle + framework whenever the panel opens (or the subject changes).
  $effect(() => {
    if (!app.subjectPanelOpen) { fwViewing = false; return; }
    void subj?.id;
    loadMoodle();
    loadFramework();
  });

  function onKey(e: KeyboardEvent) {
    if (!app.subjectPanelOpen) return;
    // The shared detail reader (app.detail) owns Esc while it's open.
    if (app.detail) return;
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      if (fwViewing) fwViewing = false;
      else app.closeSubjectPanel();
    }
  }

  // ── formatting helpers ────────────────────────────────────────
  // Moodle deadlines/announcements are epoch SECONDS; last_sync/framework are ms.
  function fmtMs(ms: number): string {
    if (!ms) return "—";
    try {
      return new Date(ms).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
    } catch {
      return "—";
    }
  }
  const fmtSecs = (s: number) => fmtMs(s * 1000);
  function relMs(ms: number): string {
    if (!ms) return "never";
    const mins = Math.round((Date.now() - ms) / 60000);
    if (mins < 1) return "just now";
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.round(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    return `${Math.round(hrs / 24)}d ago`;
  }
  // Plain-text preview of a Moodle HTML message (for the row's hover title).
  function stripHtml(html: string): string {
    return html
      .replace(/<[^>]*>/g, " ").replace(/&nbsp;/g, " ").replace(/&amp;/g, "&")
      .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&#39;/g, "'")
      .replace(/&quot;/g, '"').replace(/\s+/g, " ").trim();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if app.subjectPanelOpen && subj}
  <div class="sp-back" role="presentation" onmousedown={() => app.closeSubjectPanel()}>
    <div class="sp" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
      <!-- Header -->
      <div class="sp-head">
        <span class="sp-glyph" style="color:{app.subjectColor(subj)}">{subj.glyph || "◆"}</span>
        <div class="sp-titles">
          <div class="sp-name">{subj.name}</div>
          <div class="sp-sub mono">
            {subj.code ? subj.code + " · " : ""}{subj.sourceCount}
            {subj.sourceCount === 1 ? "source" : "sources"} · {subj.topics.length}
            {subj.topics.length === 1 ? "topic" : "topics"}
          </div>
        </div>
        <button class="btn btn--sm btn--ghost" onclick={editSubject}><Icon name="pencil" size={12} /> Edit</button>
        <button class="btn btn--icon btn--sm btn--ghost" title="Close" onclick={() => app.closeSubjectPanel()}>
          <Icon name="x" size={14} />
        </button>
      </div>

      <div class="sp-body">
        <!-- University portal (Moodle) -->
        <section class="sp-card">
          <div class="sp-card-h">
            <Icon name="link" size={14} /><span>University portal</span>
            <div class="grow"></div>
            {#if mdStatus.configured && linkedCourse}
              <span class="sp-faint mono sm">synced {relMs(mdStatus.last_sync)}</span>
              <button class="btn btn--sm" onclick={syncNow} disabled={syncing}>
                <Icon name="refresh" size={12} /> {syncing ? "Syncing…" : "Sync"}
              </button>
            {/if}
          </div>

          {#if !mdStatus.configured}
            <div class="sp-empty">
              <p class="sp-faint">Connect your Moodle portal to pull grades, deadlines and announcements into this subject.</p>
              <button class="btn btn--sm btn--primary" onclick={() => { app.closeSubjectPanel(); app.openSettings("experimental"); }}>
                <Icon name="settings" size={12} /> Open Moodle settings
              </button>
            </div>
          {:else if !linkedCourse}
            <div class="sp-empty">
              <p class="sp-faint">This subject isn't linked to a Moodle course yet.</p>
              <div class="sp-link-row">
                <div style:flex="1">
                  <Picker value="" onChange={linkCourse} options={courseOptions} placeholder={courseOptions.length ? "Link a Moodle course…" : "No courses — sync in Settings first"} />
                </div>
                <button class="btn btn--sm" onclick={autoMatch} disabled={linking}><Icon name="bolt" size={12} /> Auto-match</button>
              </div>
            </div>
          {:else}
            <div class="sp-course mono">
              <Icon name="check" size={12} /> {linkedCourse.fullname || linkedCourse.shortname}
              <div class="grow"></div>
              {#if courseUrl}
                <button class="btn btn--sm" onclick={openCourse} title="Open this course in Moodle">
                  <Icon name="external" size={12} /> Open in Moodle
                </button>
              {/if}
              <button class="sp-unlink" onclick={unlinkCourse} title="Unlink course">unlink</button>
            </div>
            <div class="sp-grid">
              <div class="sp-sub-card">
                <div class="sp-sub-h mono"><Icon name="chart" size={12} /> Grades <span class="sp-faint">· {courseGrades.length}</span></div>
                {#if courseGrades.length === 0}<p class="sp-faint sm">No grades synced.</p>{:else}
                  <ul class="sp-list">
                    {#each courseGrades as g (g.course_id + g.item_name)}
                      <li><span class="sp-li-name" title={g.item_name}>{g.item_name}</span><span class="sp-li-val mono">{g.percentage || g.grade || "—"}</span></li>
                    {/each}
                  </ul>
                {/if}
              </div>
              <div class="sp-sub-card">
                <div class="sp-sub-h mono"><Icon name="calendar" size={12} /> Deadlines <span class="sp-faint">· {courseDeadlines.length}</span></div>
                {#if courseDeadlines.length === 0}<p class="sp-faint sm">Nothing due.</p>{:else}
                  <ul class="sp-list">
                    {#each courseDeadlines.slice(0, 8) as d (d.id)}
                      <li>
                        <button class="sp-row-link" onclick={() => openDeadline(d)} title={d.name}>
                          <span class="sp-li-name">{d.name}</span>
                          <span class="sp-li-val mono" class:sp-exam={d.kind === "exam"}>{fmtSecs(d.due_at)}</span>
                        </button>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
              <div class="sp-sub-card">
                <div class="sp-sub-h mono"><Icon name="chat" size={12} /> Announcements <span class="sp-faint">· {courseAnnouncements.length}</span></div>
                {#if courseAnnouncements.length === 0}<p class="sp-faint sm">No announcements.</p>{:else}
                  <ul class="sp-list">
                    {#each courseAnnouncements.slice(0, 8) as a (a.id)}
                      <li>
                        <button class="sp-ann-btn" onclick={() => openAnnouncement(a)} title={stripHtml(a.message)}>
                          <span class="sp-ann-subj">{a.subject}</span>
                          <span class="sp-faint sm mono">{fmtSecs(a.posted_at)}</span>
                        </button>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
            </div>
          {/if}
        </section>

        <!-- Calendar matching -->
        <section class="sp-card">
          <div class="sp-card-h"><Icon name="calendar" size={14} /><span>Calendar matching</span></div>
          <input class="input mono" placeholder="keywords, e.g. GenLing, GL178" bind:value={aliasInput} onblur={saveAliases} />
          <p class="sp-faint sm" style="margin:8px 0 0">Comma-separated terms that appear in your timetable events for this subject — lectures with these in the title auto-file here (no AI). Saved on blur; re-files your calendar instantly.</p>
        </section>

        <!-- Topics -->
        <section class="sp-card">
          <div class="sp-card-h">
            <Icon name="grid" size={14} /><span>Topics</span>
            <div class="grow"></div>
            <button class="btn btn--sm btn--ghost" onclick={addTopic}><Icon name="plus" size={12} /> Add topic</button>
          </div>
          {#if subj.topics.length === 0}
            <p class="sp-faint sm">No topics yet. Add one to group sources, cheatsheets and chats.</p>
          {:else}
            <div class="sp-topics">
              {#each subj.topics as t (t.id)}
                <TopicRow topic={t} subjectId={subj.id} />
              {/each}
            </div>
          {/if}
        </section>

        <!-- Module framework -->
        <section class="sp-card">
          <div class="sp-card-h">
            <Icon name="doc" size={14} /><span>Module framework</span>
            <div class="grow"></div>
            {#if framework}
              <button class="btn btn--sm btn--ghost" onclick={uploadFramework} disabled={fwBusy}><Icon name="upload" size={12} /> Replace</button>
              <button class="btn btn--sm btn--ghost" onclick={removeFramework}><Icon name="x" size={12} /> Remove</button>
            {/if}
          </div>
          {#if framework}
            <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
            <div class="sp-fw" role="button" tabindex="0" title="Open framework" onclick={viewFramework}
                 onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); viewFramework(); } }}>
              <Icon name="doc" size={14} />
              <span class="mono sp-fw-name">{framework.filename}</span>
              <span class="sp-faint mono sm">· {fmtMs(framework.updated_at)}</span>
              <div class="grow"></div>
              <span class="sp-open mono sm"><Icon name="external" size={11} /> view</span>
            </div>
            <p class="sp-faint sm">Chat uses this for mark/weighting questions — e.g. <em>"what is my A2 weighted?"</em>. Ignored in normal chat.</p>
          {:else}
            <div class="sp-empty">
              <p class="sp-faint">Upload your module framework (course outline with assessment weights). Chat references it only when you explicitly ask about marks or weighting.</p>
              <button class="btn btn--sm btn--primary" onclick={uploadFramework} disabled={fwBusy}>
                <Icon name="upload" size={12} /> {fwBusy ? "Reading…" : "Upload framework"}
              </button>
            </div>
          {/if}
        </section>
      </div>
    </div>

    <!-- Framework document viewer (PDF / image / text) -->
    {#if fwViewing && framework}
      <div class="fw-back" role="presentation" onmousedown={(e) => { e.stopPropagation(); fwViewing = false; }}>
        <div class="fw-view" role="dialog" aria-modal="true" tabindex="-1" onmousedown={(e) => e.stopPropagation()}>
          <div class="fw-view-h">
            <Icon name="doc" size={13} />
            <span class="mono">{framework.filename}</span>
            <div class="grow"></div>
            <button class="btn btn--icon btn--sm btn--ghost" title="Close" onclick={() => (fwViewing = false)}>
              <Icon name="x" size={14} />
            </button>
          </div>
          {#if framework.view_kind === "pdf" && fwSrc}
            <iframe class="fw-frame" src={fwSrc} title={framework.filename}></iframe>
          {:else if framework.view_kind === "image" && fwSrc}
            <div class="fw-img-wrap"><img class="fw-img" src={fwSrc} alt={framework.filename} /></div>
          {:else}
            <pre class="fw-text">{fwText ?? "Loading…"}</pre>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .sp-back {
    /* Below the shared EditModal/confirm Dialog (z-index 200) so editing or
       deleting a subject/topic from here surfaces ABOVE the panel, not under it. */
    position: fixed; inset: 0; z-index: 150;
    display: flex; align-items: center; justify-content: center;
    background: color-mix(in oklab, var(--bg) 62%, transparent);
    backdrop-filter: blur(3px); animation: sp-fade 0.12s ease;
  }
  .sp {
    width: min(760px, calc(100vw - 48px)); max-height: calc(100vh - 64px);
    display: flex; flex-direction: column;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px); box-shadow: 0 18px 50px rgba(0,0,0,0.5);
    animation: sp-pop 0.13s ease;
  }
  .sp-head { display: flex; align-items: center; gap: 12px; padding: 16px 18px; border-bottom: 1px solid var(--border); }
  .sp-glyph { font-size: 24px; line-height: 1; }
  .sp-titles { min-width: 0; flex: 1; }
  .sp-name { font-size: var(--r-lg, 18px); color: var(--fg-bright); font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sp-sub { color: var(--fg-faint); font-size: 12px; margin-top: 2px; }
  .sp-body { overflow-y: auto; padding: 16px 18px 22px; display: flex; flex-direction: column; gap: 14px; }

  .sp-card { background: var(--surface-2); border: 1px solid var(--border); border-radius: var(--rad-3, 8px); padding: 14px 16px; }
  .sp-card-h { display: flex; align-items: center; gap: 8px; font-weight: 500; color: var(--fg-bright); margin-bottom: 10px; }
  .grow { flex: 1; }
  .sp-faint { color: var(--fg-faint); }
  .sm { font-size: 12px; }

  .sp-empty { display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .sp-empty p { margin: 0; }
  .sp-link-row { display: flex; gap: 8px; width: 100%; align-items: center; }

  .sp-course { display: flex; align-items: center; gap: 8px; color: var(--accent); font-size: 13px; margin-bottom: 12px; }
  .sp-unlink { background: none; border: none; color: var(--fg-faint); cursor: pointer; font-size: 11px; text-decoration: underline; padding: 0 0 0 4px; }
  .sp-unlink:hover { color: var(--fg-bright); }

  .sp-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 10px; }
  .sp-sub-card { background: var(--surface); border: 1px solid var(--border); border-radius: var(--rad-2, 6px); padding: 11px; min-height: 80px; }
  .sp-sub-h { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--fg-bright); margin-bottom: 8px; }
  .sp-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .sp-list li { display: flex; align-items: baseline; gap: 8px; font-size: 12px; }
  .sp-li-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sp-li-val { color: var(--fg-bright); white-space: nowrap; }
  .sp-exam { color: #e0708a; }
  .sp-ann-subj { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sp-ann-btn {
    display: flex; align-items: baseline; gap: 8px; width: 100%;
    background: none; border: none; padding: 3px 4px; margin: 0 -4px; border-radius: var(--rad-2, 6px);
    font: inherit; font-size: 12px; color: inherit; text-align: left; cursor: pointer;
  }
  .sp-ann-btn:hover { background: var(--surface); color: var(--fg-bright); }
  .sp-ann-btn:hover .sp-ann-subj { color: var(--accent); }

  .sp-row-link { display: flex; align-items: baseline; gap: 8px; width: 100%; color: inherit; text-decoration: none; padding: 2px 4px; margin: 0 -4px; border-radius: var(--rad-2, 6px); background: none; border: none; cursor: pointer; font: inherit; text-align: left; }
  .sp-row-link:hover { background: var(--surface); }
  .sp-row-link:hover .sp-li-name { color: var(--accent); }

  .sp-topics { display: flex; flex-direction: column; gap: 8px; }

  .sp-fw { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border: 1px solid var(--border); border-radius: var(--rad-2, 6px); background: var(--surface); cursor: pointer; transition: border-color 0.12s, background 0.12s; }
  .sp-fw:hover { border-color: var(--border-strong); background: var(--surface-3, var(--surface-2)); }
  .sp-fw-name { color: var(--fg-bright); font-size: 13px; }
  .sp-open { color: var(--accent); display: inline-flex; align-items: center; gap: 4px; }

  /* Document viewer overlay */
  .fw-back { position: fixed; inset: 0; z-index: 210; display: flex; align-items: center; justify-content: center; background: color-mix(in oklab, var(--bg) 75%, transparent); backdrop-filter: blur(4px); animation: sp-fade 0.12s ease; }
  .fw-view { width: min(1100px, calc(100vw - 48px)); height: calc(100vh - 56px); display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--border-strong); border-radius: var(--r-lg, 12px); box-shadow: 0 18px 50px rgba(0,0,0,0.55); overflow: hidden; }
  .fw-view-h { display: flex; align-items: center; gap: 8px; padding: 10px 14px; border-bottom: 1px solid var(--border); color: var(--fg-bright); }
  .fw-frame { flex: 1; width: 100%; border: 0; background: #fff; }
  .fw-img-wrap { flex: 1; overflow: auto; display: flex; align-items: flex-start; justify-content: center; padding: 16px; }
  .fw-img { max-width: 100%; height: auto; }
  .fw-text { flex: 1; margin: 0; overflow: auto; padding: 18px 22px; white-space: pre-wrap; word-break: break-word; font-family: var(--font-mono); font-size: 12.5px; line-height: 1.6; color: var(--fg); }

  @keyframes sp-fade { from { opacity: 0; } to { opacity: 1; } }
  @keyframes sp-pop { from { opacity: 0; transform: translateY(6px) scale(0.98); } to { opacity: 1; transform: none; } }
</style>
