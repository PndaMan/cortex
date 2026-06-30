// Module-level reactive store for background AI generation jobs.
//
// The whole point of this store living OUTSIDE any Svelte component is
// persistence: `start()` kicks off an async `run()` and keeps the promise
// chain here in the module, not tied to a component instance. So when the user
// navigates away (unmounting the view that triggered it), the generation keeps
// running — when it settles it updates `jobs.list` (reactive) and fires
// `onDone`. Any view that re-renders a GeneratingCard for `jobs.running(...)`
// will see the live status, even after navigating back.

import { app } from "./store.svelte";
import { notifyNow } from "./notifications";

export type JobKind =
  | "cheatsheet"
  | "quiz"
  | "flashcards"
  | "audio"
  | "infographic"
  | "slideshow"
  | "mindmap"
  | "source";

export type JobStatus = "running" | "done" | "error";

export type Job = {
  id: string;
  kind: JobKind;
  label: string;
  subjectId: string;
  topicId: string | null;
  status: JobStatus;
  error?: string;
  startedAt: number;
  result?: unknown;
};

function uid() {
  return Math.random().toString(36).slice(2);
}

const MAX_KEPT = 20; // cap settled (done/error) jobs so the list can't grow unbounded

const KIND_LABEL: Record<JobKind, string> = {
  cheatsheet: "cheatsheet",
  quiz: "quiz",
  flashcards: "flashcards",
  audio: "audio overview",
  infographic: "infographic",
  slideshow: "slideshow",
  mindmap: "mind map",
  source: "source",
};

export function jobKindLabel(kind: JobKind): string {
  return KIND_LABEL[kind] ?? kind;
}

class Jobs {
  list = $state<Job[]>([]);
  // Ids the user cancelled — their eventual result is discarded (settle skipped).
  #cancelled = new Set<string>();

  /** Running jobs for a subject, optionally filtered to one kind. */
  running(subjectId: string | null, kind?: JobKind): Job[] {
    if (!subjectId) return [];
    return this.list.filter(
      (j) =>
        j.subjectId === subjectId &&
        j.status === "running" &&
        (kind ? j.kind === kind : true)
    );
  }

  /** All jobs (any status) for a subject. */
  forSubject(subjectId: string | null): Job[] {
    if (!subjectId) return [];
    return this.list.filter((j) => j.subjectId === subjectId);
  }

  /**
   * Start a background job. Pushes a "running" Job, runs `run()` WITHOUT the
   * caller awaiting it, then on settle flips the job to done/error, stores the
   * result, toasts, and (on success) calls `onDone`. Returns the job id.
   */
  start(opts: {
    kind: JobKind;
    label: string;
    subjectId: string;
    topicId?: string | null;
    run: () => Promise<unknown>;
    onDone?: (result: unknown) => void;
  }): string {
    const id = uid();
    const job: Job = {
      id,
      kind: opts.kind,
      label: opts.label,
      subjectId: opts.subjectId,
      topicId: opts.topicId ?? null,
      status: "running",
      startedAt: Date.now(),
    };
    this.list = [...this.list, job];

    // Fire and forget — the promise chain is held by the module, not a view.
    opts
      .run()
      .then((result) => {
        if (this.#cancelled.delete(id)) return; // user cancelled — discard result
        this.#settle(id, (j) => ({ ...j, status: "done", result }));
        app.pushToast({
          kind: "success",
          title: opts.kind === "source" ? "Source added" : "Generated",
          body: `${opts.label} is ready.`,
        });
        if (opts.kind !== "source") {
          notifyNow("material", `🪄 ${opts.label} ready`, "Your generated study material is ready in Cortex.");
        }
        try {
          opts.onDone?.(result);
        } catch {
          /* view-side reload failure shouldn't crash the job */
        }
        // Adding a source no longer auto-regenerates the cheatsheet — that fired an
        // expensive LLM synthesis on every ingest. Regenerate manually from the
        // cheatsheet view (or the "Regenerate cheatsheet" command) when ready.
      })
      .catch((e: unknown) => {
        if (this.#cancelled.delete(id)) return; // user cancelled — ignore error too
        const msg = e instanceof Error ? e.message : String(e);
        this.#settle(id, (j) => ({ ...j, status: "error", error: msg }));
        app.pushToast({ kind: "error", title: "Generation failed", body: msg });
      });

    return id;
  }

  dismiss(id: string) {
    this.list = this.list.filter((j) => j.id !== id);
  }

  /** Stop tracking a running job and discard whatever it eventually returns. */
  cancel(id: string) {
    this.#cancelled.add(id);
    this.list = this.list.filter((j) => j.id !== id);
  }

  // Update one job by id and trim old settled jobs to keep the list bounded.
  #settle(id: string, update: (j: Job) => Job) {
    const next = this.list.map((j) => (j.id === id ? update(j) : j));
    const settled = next.filter((j) => j.status !== "running");
    if (settled.length > MAX_KEPT) {
      const drop = new Set(
        settled
          .slice() // oldest first
          .sort((a, b) => a.startedAt - b.startedAt)
          .slice(0, settled.length - MAX_KEPT)
          .map((j) => j.id)
      );
      this.list = next.filter((j) => !drop.has(j.id));
    } else {
      this.list = next;
    }
  }
}

export const jobs = new Jobs();
