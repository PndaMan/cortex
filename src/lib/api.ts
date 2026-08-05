// Typed Tauri command client. Mirrors src-tauri/src/commands.rs 1:1.
// Every backend command is wrapped here so views never call `invoke` directly.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Source {
  id: string;
  subject_id: string;
  topic_id: string | null;
  name: string;
  kind: string;
  status: string;
  meta: string | null;
  origin: string | null;
  error: string | null;
  content: string | null;      // extracted plaintext (for readable text preview)
  stored_path: string | null;  // stable path to the persisted original/rendered file
  tags: string[];
  created_at: number;
  updated_at: number;
}

export interface Topic {
  id: string;
  subject_id: string;
  name: string;
  glyph: string | null;
  position: number;
  tags: string[];
  sources: Source[];
}

export interface Subject {
  id: string;
  name: string;
  code: string | null;
  glyph: string;
  color: string | null;
  status: string; // ready | review
  streak: number;
  position: number;
  sourceCount: number;
  topics: Topic[];
  created_at: number;
  updated_at: number;
  moodle_course_id?: string | null;
  calendar_aliases?: string | null;
  archived?: boolean;
}

export interface IngestResult {
  source: Source;
  chunk_count: number;
  chars: number;
  warning: string | null;
}

export interface ChunkInfo {
  ord: number;
  text: string;
  dim: number;
  loc: string | null;
}

export interface ChunkHit {
  id: string;
  source_id: string;
  source_name: string;
  text: string;
  loc: string | null;
  score: number;
}

/** One global-search (Ctrl+K) result. For "chunk" hits `id` is the source id. */
export interface SearchHit {
  kind: "chunk" | "source" | "note" | "event" | "material";
  id: string;
  subject_id: string | null;
  title: string;
  snippet: string;
  score: number;
}

export interface IngestProgress {
  source_id: string;
  stage: string; // parsing | chunking | embedding | storing | done | error
  detail: string;
  pct: number;
}

export interface Citation {
  source_name: string;
  loc: string | null;
  snippet: string;
}
export interface WebImage {
  img: string;
  thumb: string;
  title: string;
  source: string;
}
export interface ChatAnswer {
  text: string;
  citations: Citation[];
  model: string;
  images?: WebImage[];
}
export interface CsItem { t: string; d: string }
export interface CsSection { id: string; title: string; state: string; items: CsItem[]; image?: string | null }
export interface CheatsheetData {
  subject: string;
  topic: string;
  sources: number;
  /** Sources actually synthesized into this sheet (coverage). */
  sources_used: number;
  model: string;
  sections: CsSection[];
}
export interface CheatsheetVersionMeta {
  id: string;
  created_at: number;
  note: string;
  section_count: number;
}
export interface MaterialRec {
  id: string;
  kind: string;
  title: string;
  topic: string;
  meta: string;
  status: string;
  payload: any;
}

export interface AddSourceInput {
  subject_id: string;
  topic_id?: string | null;
  name?: string | null;
  kind?: string | null;
  text?: string | null;
  path?: string | null;
  url?: string | null;
  tags?: string[];
}

// ---- subjects ----
export const listSubjects = () => invoke<Subject[]>("list_subjects");
export const getSubject = (id: string) => invoke<Subject>("get_subject", { id });
export const createSubject = (name: string, code?: string, glyph?: string, color?: string) =>
  invoke<Subject>("create_subject", { name, code, glyph, color });
export const updateSubject = (
  id: string,
  name: string,
  code?: string,
  glyph?: string,
  color?: string
) => invoke<Subject>("update_subject", { id, name, code, glyph, color });
export const deleteSubject = (id: string) => invoke<void>("delete_subject", { id });
export const archiveSubject = (id: string, archived: boolean) =>
  invoke<void>("archive_subject", { id, archived });
export const listArchivedSubjects = () =>
  invoke<Subject[]>("list_archived_subjects", {});

// ---- topics ----
export const createTopic = (subjectId: string, name: string, glyph?: string, tags?: string[]) =>
  invoke<Subject>("create_topic", { subjectId, name, glyph, tags });
export const updateTopic = (id: string, name: string, subjectId: string, glyph?: string, tags?: string[]) =>
  invoke<Subject>("update_topic", { id, name, subjectId, glyph, tags });
export const reorderSubjects = (ids: string[]) =>
  invoke<Subject[]>("reorder_subjects", { ids });
export const reorderTopics = (subjectId: string, ids: string[]) =>
  invoke<Subject>("reorder_topics", { subjectId, ids });
export const deleteTopic = (id: string, subjectId: string) =>
  invoke<Subject>("delete_topic", { id, subjectId });

// ---- sources ----
export const listSources = (subjectId: string) =>
  invoke<Source[]>("list_sources", { subjectId });
export const getSource = (id: string) => invoke<Source>("get_source", { id });
export const updateSource = (
  id: string,
  name: string,
  topicId?: string | null,
  tags?: string[]
) => invoke<Source>("update_source", { id, name, topicId, tags });
export const deleteSource = (id: string) => invoke<void>("delete_source", { id });
/** Re-run ingestion (re-OCR/re-chunk/re-embed) for an existing source in place. */
export const reingestSource = (id: string) => invoke<IngestResult>("reingest_source", { id });
/** Sources that failed to ingest (error / draft-with-error), across all subjects. */
export const listFailedSources = () => invoke<Source[]>("list_failed_sources");

// ---- live homelab sync (smart row-level merge + binary file sync) ----
export interface SyncStatus {
  enabled: boolean;
  configured: boolean;
  last_at: number; // epoch-ms, 0 = never
  /** Live-sync WebSocket connected — changes propagate within ~a second. */
  live: boolean;
}
export const syncStatus = () => invoke<SyncStatus>("sync_status");
export const syncTest = (url: string, user: string, pass: string) =>
  invoke<boolean>("sync_test", { url, user, pass });
/** Push the local DB (union snapshot) + binary files to the homelab; returns the push timestamp (ms). */
export const syncPush = () => invoke<number>("sync_push");
/** Pull + merge the remote vault into the local DB; returns true if a newer remote was merged. */
export const syncPull = () => invoke<boolean>("sync_pull");

// ---- Moodle integration (experimental) ----
export interface MoodleStatus { configured: boolean; user_id: number; last_sync: number }
export interface MoodleSummary { courses: number; grades: number; deadlines: number; announcements: number }
export interface MoodleCourse { id: string; shortname: string; fullname: string }
export interface MoodleGrade { course_id: string; item_name: string; grade: string; percentage: string; feedback: string }
export interface MoodleDeadline { id: string; course_id: string; name: string; due_at: number; kind: string; status: string; url: string }
export interface MoodleAnnouncement { id: string; course_id: string; subject: string; message: string; posted_at: number; url: string }
export interface MoodleData { courses: MoodleCourse[]; grades: MoodleGrade[]; deadlines: MoodleDeadline[]; announcements: MoodleAnnouncement[] }
/** Connect with username+password (non-SSO). Returns the user's full name. */
export const moodleConnect = (url: string, username: string, password: string) =>
  invoke<string>("moodle_connect", { url, username, password });
/** Connect with a pasted web-services token (SSO sites). Returns the user's full name. */
export const moodleSetToken = (url: string, token: string) =>
  invoke<string>("moodle_set_token", { url, token });
/** Open the Moodle SSO launch flow in a window; token captured via the callback. */
export const moodleLoginSso = (url: string) => invoke<void>("moodle_login_sso", { url });
/** Fired when the SSO login window captures + stores a token. Payload: user's name. */
export const onMoodleSsoDone = (cb: (name: string) => void): Promise<UnlistenFn> =>
  listen<string>("moodle-sso-done", (e) => cb(e.payload));
/** Fired when the SSO login flow fails. Payload: error message. */
export const onMoodleSsoError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
  listen<string>("moodle-sso-error", (e) => cb(e.payload));
export const moodleStatus = () => invoke<MoodleStatus>("moodle_status");
export const moodleDisconnect = () => invoke<void>("moodle_disconnect");
export const moodleSync = () => invoke<MoodleSummary>("moodle_sync");
export const moodleData = () => invoke<MoodleData>("moodle_data");
export const moodleLinkSubject = (subjectId: string, courseId: string | null) =>
  invoke<void>("moodle_link_subject", { subjectId, courseId });
export const moodleAutolink = () => invoke<number>("moodle_autolink");

// ---- calendar event → subject matching (no AI) ----
export const setSubjectAliases = (subjectId: string, aliases: string) =>
  invoke<number>("set_subject_aliases", { subjectId, aliases });
export const retagCalendarEvents = () => invoke<number>("retag_calendar_events");

// ---- external dependency status ----
export interface DepStatus { name: string; detail: string; present: boolean }
export interface DependencyReport { manager: string; deps: DepStatus[]; install_command: string; note: string }
export const dependencyStatus = () => invoke<DependencyReport>("dependency_status");
export const installDependencies = () => invoke<string>("install_dependencies");
/** "appimage" | "linux-package" | "macos" | "windows" | "unknown" — how Cortex was installed. */
export const installKind = () => invoke<string>("install_kind");
export interface FolderFile { path: string; name: string }
export const listFolderSources = (dir: string) => invoke<FolderFile[]>("list_folder_sources", { dir });

// ---- per-subject module framework ----
export interface FrameworkMeta {
  filename: string;
  chars: number;
  updated_at: number;
  file_path: string | null;
  view_kind: string; // pdf | image | text
}
export const setSubjectFramework = (subjectId: string, path: string) =>
  invoke<FrameworkMeta>("set_subject_framework", { subjectId, path });
export const getSubjectFramework = (subjectId: string) =>
  invoke<FrameworkMeta | null>("get_subject_framework", { subjectId });
export const getSubjectFrameworkText = (subjectId: string) =>
  invoke<string | null>("get_subject_framework_text", { subjectId });
export const clearSubjectFramework = (subjectId: string) =>
  invoke<void>("clear_subject_framework", { subjectId });
export const listChunks = (sourceId: string) =>
  invoke<ChunkInfo[]>("list_chunks", { sourceId });
export const addSource = (input: AddSourceInput) =>
  invoke<IngestResult>("add_source", { input });
/** Copy a just-picked file into app storage and return the stable path. On mobile the
 *  picker's temp path is deleted before the background ingest runs — stage it first. */
export const stageUpload = (path: string) =>
  invoke<string>("stage_upload", { path });

// ---- search / settings / demo ----
export const searchChunks = (query: string, subjectId?: string, k?: number) =>
  invoke<ChunkHit[]>("search_chunks", { query, subjectId, k });
/** Global Ctrl+K search: semantic across all subjects + text over records. */
export const globalSearch = (query: string) =>
  invoke<SearchHit[]>("global_search", { query });
export const getSetting = (key: string) => invoke<string | null>("get_setting", { key });
export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });
export const seedDemo = () => invoke<Subject[]>("seed_demo");
export const envProbe = () => invoke<Record<string, boolean>>("env_probe");

// ---- AI ----
export const chatAnswer = (
  subjectId: string,
  level: "subject" | "topic" | "source",
  query: string,
  sourceId?: string,
  sourceIds?: string[],
  web?: boolean
) => invoke<ChatAnswer>("chat_answer", { subjectId, level, query, sourceId, sourceIds, web });
export const generateCheatsheet = (subjectId: string, topicId?: string, withImages?: boolean) =>
  invoke<CheatsheetData>("generate_cheatsheet", { subjectId, topicId, withImages });
/** Regenerate every topic's sheet (+ ungrouped "General"), then return the composed whole-subject sheet. */
export const generateSubjectCheatsheet = (subjectId: string, withImages?: boolean) =>
  invoke<CheatsheetData | null>("generate_subject_cheatsheet", { subjectId, withImages });
export const getCheatsheet = (subjectId: string, topicId?: string) =>
  invoke<CheatsheetData | null>("get_cheatsheet", { subjectId, topicId });
/** Whole-subject sheet, composed from the stored per-topic sheets. */
export const getSubjectCheatsheet = (subjectId: string) =>
  invoke<CheatsheetData | null>("get_subject_cheatsheet", { subjectId });
export const updateCheatsheet = (subjectId: string, topicId: string | undefined, sections: CsSection[], snapshot = true) =>
  invoke<void>("update_cheatsheet", { subjectId, topicId, sections, snapshot });
export const listCheatsheetVersions = (subjectId: string, topicId?: string) =>
  invoke<CheatsheetVersionMeta[]>("list_cheatsheet_versions", { subjectId, topicId });
export const getCheatsheetVersion = (versionId: string) =>
  invoke<CsSection[]>("get_cheatsheet_version", { versionId });
/** Restore a stored version as the live sheet (snapshots current first). Returns the restored sheet. */
export const restoreCheatsheetVersion = (versionId: string) =>
  invoke<CheatsheetData>("restore_cheatsheet_version", { versionId });
/** Render a self-contained HTML doc to a PDF at `dest` (headless Chromium). */
export const exportPdf = (html: string, dest: string) =>
  invoke<void>("export_pdf", { html, dest });
/** Copy the whole database to a portable .db file at `dest`. */
export const exportDatabase = (dest: string) =>
  invoke<void>("export_database", { dest });
/** Export a flashcard material to an Anki `.apkg` deck at `dest`; returns card count. */
export const exportAnki = (materialId: string, dest: string) =>
  invoke<number>("export_anki", { materialId, dest });
/** Summary of an Anki `.apkg` import: decks created, cards stored, cards skipped. */
export interface AnkiImportResult {
  deck_count: number;
  card_count: number;
  skipped: number;
}
/** Import an Anki `.apkg` at `path` into a subject as flashcard materials (one
 *  per deck). HTML-stripped, deduped vs existing decks in the subject. */
export const importAnki = (subjectId: string, path: string, topicId?: string) =>
  invoke<AnkiImportResult>("import_anki", { subjectId, topicId, path });
// ---- encrypted homelab backups (age + rclone) ----
export interface BackupStatus {
  age_found: boolean;
  rclone_found: boolean;
  recipient_set: boolean;
  remote_set: boolean;
  last_at: number | null;
  last_dest: string | null;
}
export const backupStatus = () => invoke<BackupStatus>("backup_status");
/** Snapshot → age-encrypt → rclone-upload; returns the remote destination path. */
export const backupNow = () => invoke<string>("backup_now");
/** Reclaim disk space (WAL checkpoint + VACUUM). */
export const optimizeDb = () => invoke<void>("optimize_db", {});
export const generateMaterial = (
  subjectId: string,
  kind: "flashcards" | "quiz" | "audio" | "infographic" | "slideshow" | "mindmap",
  topicId?: string,
  title?: string,
  customPrompt?: string,
  sourceIds?: string[],
  count?: number
) => invoke<MaterialRec>("generate_material", { subjectId, kind, topicId, title, customPrompt, sourceIds, count });
/** Synthesize a real audio-overview mp3 from the script segments via cloud TTS.
 *  Returns the file path (serve to <audio> with convertFileSrc). Errors offline /
 *  without an OpenAI key — caller falls back to on-device speech synthesis. */
export const synthesizeOverview = (
  materialId: string,
  segments: { speaker: string; text: string }[],
  force?: boolean
) => invoke<string>("synthesize_overview", { materialId, segments, force });
export const listMaterials = (subjectId: string) =>
  invoke<MaterialRec[]>("list_materials", { subjectId });
export const deleteMaterial = (id: string) => invoke<void>("delete_material", { id });
export const renameMaterial = (id: string, title: string) =>
  invoke<void>("rename_material", { id, title });

// ---- exam mode (timed, locally-graded practice exams) ----
// `questions`/`answers`/`results` are loosely-typed JSON (mirrors the backend's
// serde_json::Value); ExamView narrows them at the point of use.
export interface ExamRec {
  id: string;
  subject_id: string;
  topic_ids: string[];
  title: string;
  duration_min: number;
  questions: any;
  answers: any;
  results: any;
  status: string; // ready | in_progress | graded
  started_ms: number | null;
  score: number | null;
  created_at: number;
  updated_at: number;
}
export interface ExamAnswerInput {
  id: string;
  choice?: number | null;
  text?: string | null;
}
export const generateExam = (
  subjectId: string,
  topicIds: string[] | undefined,
  durationMin: number,
  mcqCount: number,
  writtenCount: number
) =>
  invoke<ExamRec>("generate_exam", { subjectId, topicIds, durationMin, mcqCount, writtenCount });
export const startExam = (id: string) => invoke<ExamRec>("start_exam", { id });
export const submitExam = (id: string, answers: ExamAnswerInput[]) =>
  invoke<any>("submit_exam", { id, answers });
/** Re-grade a finished exam's stored answers (same pipeline/rubric as submit). */
export const remarkExam = (id: string) => invoke<any>("remark_exam", { id });
export const listExams = (subjectId: string) =>
  invoke<ExamRec[]>("list_exams", { subjectId });
export const getExam = (id: string) => invoke<ExamRec>("get_exam", { id });
export const deleteExam = (id: string) => invoke<void>("delete_exam", { id });

// ---- lecture recording (Whisper) ----
export const saveRecording = (
  subjectId: string,
  name: string,
  audio: number[],
  topicId?: string,
  ext?: string
) => invoke<IngestResult>("save_recording", { subjectId, name, audio, topicId, ext });

/** Raw-bytes save: the audio rides the invoke body as-is (no JSON number[]),
 * which is the only sane transport for hour-long recordings (~100MB). Metadata
 * travels in headers (percent-encoded — header values must be ASCII-safe). */
export const saveRecordingRaw = (
  subjectId: string,
  name: string,
  audio: Uint8Array,
  topicId?: string,
  ext?: string,
  diarize?: boolean
) =>
  invoke<IngestResult>("save_recording_raw", audio, {
    headers: {
      "x-subject-id": encodeURIComponent(subjectId),
      "x-name": encodeURIComponent(name),
      ...(topicId ? { "x-topic-id": encodeURIComponent(topicId) } : {}),
      ...(ext ? { "x-ext": encodeURIComponent(ext) } : {}),
      ...(diarize !== undefined ? { "x-diarize": String(diarize) } : {}),
    },
  });

// Near-live transcription: transcribe an audio slice and return its text (or ""
// if no Whisper transcriber is installed). Used by the recorder's live panel.
export const transcribePartial = (audio: number[], ext?: string) =>
  invoke<string>("transcribe_partial", { audio, ext });

// Validate the homelab Whisper setup end to end: server reachable + configured
// model installed (downloading it on the spot if missing). Resolves to a
// human-readable status; rejects with what to fix.
export const checkWhisperModel = () => invoke<string>("check_whisper_model");

// ---- native (iOS) lecture recording ----
// WKWebView's custom-scheme pages are not a secure context, so getUserMedia is
// unavailable on iOS — capture runs natively (AVAudioRecorder) behind these
// commands instead, and keeps recording while the phone is locked.
export const nativeRecStart = () => invoke<void>("native_rec_start");
export const nativeRecPause = () => invoke<void>("native_rec_pause");
export const nativeRecResume = () => invoke<void>("native_rec_resume");
/** Stop and return the recorded file's path + duration (secs). */
export const nativeRecStop = () =>
  invoke<{ path: string; secs: number }>("native_rec_stop");
export const nativeRecCancel = () => invoke<void>("native_rec_cancel");
/** Metering sample: input level 0..1 for the waveform + authoritative elapsed
 * seconds (webview timers freeze while the phone is locked; the recorder's
 * clock doesn't). */
export const nativeRecLevel = () =>
  invoke<{ level: number; secs: number }>("native_rec_level");
/** Delete a stopped-but-unsaved native recording file. */
export const nativeRecDiscardFile = (path: string) =>
  invoke<void>("native_rec_discard", { path });
/** Save a recording that already lives in a backend file (native capture path). */
export const saveRecordingPath = (
  subjectId: string,
  name: string,
  path: string,
  topicId?: string,
  diarize?: boolean
) => invoke<IngestResult>("save_recording_path", { subjectId, name, path, topicId, diarize });

// ---- settings (bulk) ----
export const getAllSettings = () => invoke<Record<string, string>>("get_all_settings");
export const setSettings = (values: Record<string, string>) =>
  invoke<void>("set_settings", { values });

// ---- chat history (persisted per subject) ----
export interface ChatMsg { role: string; text: string; created_at: number }
export const listChatMessages = (subjectId: string) =>
  invoke<ChatMsg[]>("list_chat_messages", { subjectId });
export const addChatMessage = (subjectId: string, role: string, text: string) =>
  invoke<void>("add_chat_message", { subjectId, role, text });
export const clearChat = (subjectId: string) => invoke<void>("clear_chat", { subjectId });

// chat sessions (history)
export interface ThreadInfo { id: string; title: string; updated_at: number; count: number }
export const newChat = (subjectId: string) => invoke<void>("new_chat", { subjectId });
export const listChatThreads = (subjectId: string) =>
  invoke<ThreadInfo[]>("list_chat_threads", { subjectId });
export const openChatThread = (subjectId: string, threadId: string) =>
  invoke<void>("open_chat_thread", { subjectId, threadId });

// ---- web search (SearXNG on the user's homelab) ----
export interface WebResult {
  title: string;
  url: string;
  host: string;
  snippet: string;
  engine: string;
  img_src?: string | null;
  thumbnail?: string | null;
}
export const webSearch = (query: string, categories?: string) =>
  invoke<WebResult[]>("web_search", { query, categories });

// Current Omarchy theme name (null if Omarchy isn't installed). Powers the
// "Follow Omarchy theme" toggle in Appearance.
export const omarchyTheme = () => invoke<string | null>("omarchy_theme");

// ---- long-term memory (manual; injected into AI prompts) ----
export interface Memory {
  id: string;
  content: string;
  source: string | null;
  created_at: number;
  updated_at: number;
}
export const listMemory = () => invoke<Memory[]>("list_memory");
export const addMemory = (content: string) => invoke<Memory>("add_memory", { content });
export const deleteMemory = (id: string) => invoke<void>("delete_memory", { id });

// ---- custom music stations + YouTube-audio streaming (mpv sidecar) ----
export interface CustomStation {
  id: string;
  name: string;
  url: string;
  kind: string; // "youtube" | "live"
  position: number;
  created_at: number;
}
export interface MediaTools {
  mpv: boolean;
  ffmpeg: boolean;
  ytdlp: boolean;
  ytdlp_path: string;
}
export const listCustomStations = () => invoke<CustomStation[]>("list_custom_stations");
export const addCustomStation = (name: string, url: string, kind = "youtube") =>
  invoke<CustomStation>("add_custom_station", { name, url, kind });
export const deleteCustomStation = (id: string) =>
  invoke<void>("delete_custom_station", { id });
export const reorderCustomStations = (ids: string[]) =>
  invoke<void>("reorder_custom_stations", { ids });
export const mediaToolsStatus = () => invoke<MediaTools>("media_tools_status");
export const youtubePlay = (url: string, volume: number) =>
  invoke<void>("youtube_play", { url, volume });
export const youtubePause = () => invoke<void>("youtube_pause");
export const youtubeResume = () => invoke<void>("youtube_resume");
export const youtubeStop = () => invoke<void>("youtube_stop");
export const youtubeSetVolume = (volume: number) =>
  invoke<void>("youtube_set_volume", { volume });
// Fire-and-forget: pre-resolve station URLs in the background so their first
// play is near-instant (the mpv sidecar caches the resolved direct stream).
export const youtubePrewarm = (urls: string[]) =>
  invoke<void>("youtube_prewarm", { urls });

// ---- data & privacy / homelab utilities ----
export interface DbStats {
  db_bytes: number;
  subjects: number;
  sources: number;
  chunks: number;
}
export const dbStats = () => invoke<DbStats>("db_stats");
export const deleteAllData = () => invoke<void>("delete_all_data");
export const pingUrl = (url: string) => invoke<boolean>("ping_url", { url });

/** Models actually installed on the configured Ollama server (local or homelab). Empty when unreachable / none pulled. */
export const ollamaModels = () => invoke<string[]>("ollama_models");
/** Lightweight authenticated probe of a provider's stored key/url. provider: gemini|openrouter|openai|claude|custom|ollama. */
export interface VerifyResult { ok: boolean; detail: string }
export const verifyProvider = (provider: string) =>
  invoke<VerifyResult>("verify_provider", { provider });

// ---- Google Calendar (OAuth + sync) ----
export interface GoogleStatus {
  connected: boolean;
  email: string | null;
  configured: boolean;
}
export interface SyncResult {
  pulled: number;
  pushed: number;
}
export const googleStatus = () => invoke<GoogleStatus>("google_status");
export const googleConnect = () => invoke<GoogleStatus>("google_connect");
export const googleDisconnect = () => invoke<GoogleStatus>("google_disconnect");
export const googleSync = () => invoke<SyncResult>("google_sync");
export interface GoogleCalendar { id: string; summary: string; primary: boolean; selected: boolean; color: string }
export const googleListCalendars = () => invoke<GoogleCalendar[]>("google_list_calendars");

// ---- in-app reader browsing (inside Web search) ----
export interface PageLink { href: string; text: string }
export interface FetchedPage {
  url: string;
  final_url: string;
  title: string;
  text: string;
  links: PageLink[];
}
export const fetchPage = (url: string) => invoke<FetchedPage>("fetch_page", { url });

// ---- source re-filing ----
export const moveSource = (id: string, subjectId: string, topicId?: string | null) =>
  invoke<Source>("move_source", { id, subjectId, topicId });

// ---- notes ----
export interface Note {
  id: string;
  subject_id: string | null;
  topic_id: string | null;
  title: string;
  body: string;
  source_id: string | null;
  created_at: number;
  updated_at: number;
}
export const createNote = (
  title: string,
  body: string,
  subjectId?: string | null,
  topicId?: string | null
) => invoke<Note>("create_note", { subjectId, topicId, title, body });
export const listNotes = (subjectId?: string | null) =>
  invoke<Note[]>("list_notes", { subjectId });
export const getNote = (id: string) => invoke<Note>("get_note", { id });
export const updateNote = (id: string, title: string, body: string) =>
  invoke<Note>("update_note", { id, title, body });
export const deleteNote = (id: string) => invoke<void>("delete_note", { id });
export const noteToSource = (id: string) =>
  invoke<IngestResult>("note_to_source", { id });

// ---- calendar (events + tasks) ----
export interface CalEvent {
  id: string;
  subject_id: string | null;
  title: string;
  description: string | null;
  location: string | null;
  color: string | null;
  start_ms: number;
  end_ms: number | null;
  all_day: boolean;
  kind: string; // event | task | exam | assignment | project
  done: boolean;
  reminder_ms: number | null;
  notified: boolean;
  google_id: string | null;
  tags: string[];
  checklist: string[]; // ticked topic ids (deadline study checklist)
  priority: string | null; // assignment priority: low | med | high (null = normal)
  topic_ids: string[]; // covered topic ids (assignments)
  status: string; // kanban column: todo | doing | done
  created_at: number;
  updated_at: number;
}
export const createEvent = (e: {
  title: string;
  startMs: number;
  subjectId?: string | null;
  description?: string | null;
  location?: string | null;
  color?: string | null;
  endMs?: number | null;
  allDay?: boolean;
  kind?: string;
  reminderMs?: number | null;
  tags?: string[];
  priority?: string | null;
  topicIds?: string[];
}) =>
  invoke<CalEvent>("create_event", {
    subjectId: e.subjectId,
    title: e.title,
    description: e.description,
    location: e.location,
    color: e.color,
    startMs: e.startMs,
    endMs: e.endMs,
    allDay: e.allDay,
    kind: e.kind,
    reminderMs: e.reminderMs,
    tags: e.tags,
    priority: e.priority,
    topicIds: e.topicIds,
  });
export const listEvents = (
  subjectId?: string | null,
  fromMs?: number | null,
  toMs?: number | null
) => invoke<CalEvent[]>("list_events", { subjectId, fromMs, toMs });
export const updateEvent = (e: {
  id: string;
  title: string;
  startMs: number;
  description?: string | null;
  location?: string | null;
  color?: string | null;
  endMs?: number | null;
  allDay?: boolean;
  kind?: string;
  reminderMs?: number | null;
  tags?: string[];
  /** Omit to keep the stored value; "none" clears. */
  priority?: string | null;
  /** Omit to keep the stored value. */
  topicIds?: string[];
}) =>
  invoke<CalEvent>("update_event", {
    id: e.id,
    title: e.title,
    description: e.description,
    location: e.location,
    color: e.color,
    startMs: e.startMs,
    endMs: e.endMs,
    allDay: e.allDay,
    kind: e.kind,
    reminderMs: e.reminderMs,
    tags: e.tags,
    priority: e.priority ?? undefined,
    topicIds: e.topicIds,
  });
export const deleteEvent = (id: string) => invoke<void>("delete_event", { id });
export const setEventDone = (id: string, done: boolean) =>
  invoke<CalEvent>("set_event_done", { id, done });
export const setEventStatus = (id: string, status: "todo" | "doing" | "done") =>
  invoke<CalEvent>("set_event_status", { id, status });
/** Open an http(s) URL in the system browser (webview <a target=_blank> is a no-op in Tauri). */
export const openExternal = (url: string) => invoke<void>("open_external", { url });
/** Set the deadline study checklist (ticked topic ids). */
export const setEventChecklist = (id: string, topicIds: string[]) =>
  invoke<CalEvent>("set_event_checklist", { id, topicIds });
export const checkReminders = (systemNotify = false) =>
  invoke<CalEvent[]>("check_reminders", { systemNotify });

// ---- citations (per-subject bibliography) ----
export interface Reference {
  id: string;
  subjectId: string;
  ctype: string; // article | book | web | other
  title: string;
  authors: string | null;
  year: string | null;
  container: string | null;
  url: string | null;
  doi: string | null;
  notes: string | null;
  created_at: number;
  updated_at: number;
}
export interface CitationFields {
  ctype: string;
  title: string;
  authors?: string | null;
  year?: string | null;
  container?: string | null;
  url?: string | null;
  doi?: string | null;
  notes?: string | null;
}
export const addCitation = (subjectId: string, f: CitationFields) =>
  invoke<string>("add_citation", { subjectId, ...f });
export const listCitations = (subjectId: string) =>
  invoke<Reference[]>("list_citations", { subjectId });
export const updateCitation = (id: string, f: CitationFields) =>
  invoke<void>("update_citation", { id, ...f });
export const deleteCitation = (id: string) =>
  invoke<void>("delete_citation", { id });

// ---- review (spaced repetition over wrong answers) ----
export interface Attempt {
  id: string;
  subject_id: string;
  material_id: string | null;
  kind: string; // quiz | flashcard
  item_index: number;
  item_key: string;
  correct: boolean;
  created_at: number;
}
export interface ReviewItem {
  item_index: number;
  item_key: string;
}
export const recordAttempt = (
  subjectId: string,
  kind: "quiz" | "flashcard",
  itemIndex: number,
  itemKey: string,
  correct: boolean,
  materialId?: string | null
) =>
  invoke<void>("record_attempt", {
    subjectId,
    materialId,
    kind,
    itemIndex,
    itemKey,
    correct,
  });
export const reviewSet = (subjectId: string, kind: "quiz" | "flashcard") =>
  invoke<ReviewItem[]>("review_set", { subjectId, kind });

// ---- SM-2 spaced repetition ----
export interface DueCard {
  item_index: number;
  item_key: string;
  due_at: number;
  reps: number;
  interval_d: number;
}
export interface SrsResult {
  due_at: number;
  interval_d: number;
  reps: number;
  ease: number;
}
export interface SrsStats {
  due: number;
  total: number;
}
/** Grade a card with SM-2. quality 0-5 (Again≈1, Hard≈3, Good≈4, Easy≈5). */
export const srsGrade = (
  subjectId: string,
  kind: "quiz" | "flashcard",
  itemIndex: number,
  itemKey: string,
  quality: number,
  materialId?: string | null
) =>
  invoke<SrsResult>("srs_grade", {
    subjectId,
    materialId,
    kind,
    itemIndex,
    itemKey,
    quality,
  });
/** Cards due for review now (oldest-due first). */
export const srsDue = (subjectId: string, kind: "quiz" | "flashcard") =>
  invoke<DueCard[]>("srs_due", { subjectId, kind });
/** Next interval (days) per grade: [again, hard, good, easy]. */
export const srsPreview = (subjectId: string, kind: "quiz" | "flashcard", itemKey: string) =>
  invoke<number[]>("srs_preview", { subjectId, kind, itemKey });
/** Due-now + total scheduled counts for a subject+kind. */
export const srsStats = (subjectId: string, kind: "quiz" | "flashcard") =>
  invoke<SrsStats>("srs_stats", { subjectId, kind });

// ---- study analytics ----
export interface DayMinutes { day: string; minutes: number }
export interface DayReviews { day: string; reviews: number; correct: number; accuracy: number }
export interface DueDay { day: string; due: number }
export interface SubjectStat {
  subject_id: string;
  minutes: number;
  reviews: number;
  correct: number;
  accuracy: number;
}
export interface FsrsTotals { cards: number; avg_stability: number; lapses: number }
export interface WeakTopic {
  subject_id: string;
  topic_id: string;
  topic_name: string;
  reviews: number;
  correct: number;
  accuracy: number;
  lapses: number;
  avg_stability: number;
  reason: string;
}
export interface PomodoroStats {
  focus_sessions: number;
  focus_minutes: number;
  break_minutes: number;
  avg_session_min: number;
  longest_session_min: number;
  by_hour: number[];
}
export interface AnalyticsSummary {
  minutes_per_day: DayMinutes[];
  /** A full rolling year (366 days) of daily study minutes for the heatmap. */
  year_minutes: DayMinutes[];
  reviews_per_day: DayReviews[];
  due_forecast: DueDay[];
  per_subject: SubjectStat[];
  weak_topics: WeakTopic[];
  fsrs: FsrsTotals;
  streak: number;
  pomodoro: PomodoroStats;
  minutes_week: number;
  reviews_week: number;
  accuracy_week: number;
}
/** Log a study segment: "work"/"break" (pomodoro) or "app" (passive focused time). */
export const logPomodoroSession = (
  subjectId: string | null,
  kind: "work" | "break" | "app",
  startedMs: number,
  endedMs: number
) => invoke<void>("log_pomodoro_session", { subjectId, kind, startedMs, endedMs });
/** The whole Study Analytics dashboard in one call (default 30-day window). */
export const analyticsSummary = (days?: number) =>
  invoke<AnalyticsSummary>("analytics_summary", { days });
export interface TopicStat {
  topic_id: string; topic_name: string; reviews: number; correct: number;
  accuracy: number; lapses: number; cards: number; avg_stability: number;
  sources: number; materials: number;
}
export const topicStats = (subjectId: string, days?: number) =>
  invoke<TopicStat[]>("topic_stats", { subjectId, days });

// ---- events ----
export const onIngestProgress = (
  cb: (p: IngestProgress) => void
): Promise<UnlistenFn> => listen<IngestProgress>("ingest:progress", (e) => cb(e.payload));
/** Fired by the tray menu's "Play / pause music" item. */
export const onTrayMusicToggle = (cb: () => void): Promise<UnlistenFn> =>
  listen("tray-music-toggle", () => cb());
export const onTrayGoDashboard = (cb: () => void): Promise<UnlistenFn> =>
  listen("tray-go-dashboard", () => cb());
/** Fired when a background job creates a note (e.g. auto lecture summary). */
export const onNoteCreated = (cb: () => void): Promise<UnlistenFn> =>
  listen("note:created", () => cb());
/** Fired after live sync merges peers' changes into the local DB. */
export const onSyncApplied = (cb: () => void): Promise<UnlistenFn> =>
  listen("sync:applied", () => cb());

// ---- homelab per-service status (Settings → Integrations) ----
export interface HomelabServiceStatus {
  id: string;
  label: string;
  configured: boolean;
  ok: boolean;
  detail: string;
}
/** Probe every homelab service through the URLs the app actually uses. */
export const homelabStatus = () => invoke<HomelabServiceStatus[]>("homelab_status");

// ---- notification tap deep links (mobile) ----
// Where a tapped OS notification should land in the app; stored by the backend
// (alerts.rs) keyed on the notification's numeric id.
export interface NotifRoute {
  id: number;
  /** "lecture" (→ subject) · "deadline"/"exam"/"event" (→ calendar day) */
  kind: string;
  subjectId?: string | null;
  ts?: number | null;
}
export const notificationRoute = (id: number) =>
  invoke<NotifRoute | null>("notification_route", { id });

/** Listen for OS-notification taps (mobile only — the desktop plugin has no
 *  tap events). The callback gets the tapped notification's numeric id. */
export async function onNotificationTap(cb: (id: number) => void): Promise<void> {
  const { addPluginListener } = await import("@tauri-apps/api/core");
  await addPluginListener(
    "notification",
    "actionPerformed",
    (data: { actionId?: string; notification?: { id?: number } }) => {
      // "tap" is the plain open action; explicit action buttons pass through too.
      if (data?.actionId === "dismiss") return;
      const id = data?.notification?.id;
      if (typeof id === "number") cb(id);
    }
  );
}
