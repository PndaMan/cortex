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
export interface ChatAnswer {
  text: string;
  citations: Citation[];
  model: string;
}
export interface CsItem { t: string; d: string }
export interface CsSection { id: string; title: string; state: string; items: CsItem[] }
export interface CheatsheetData {
  subject: string;
  topic: string;
  sources: number;
  model: string;
  sections: CsSection[];
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

// ---- topics ----
export const createTopic = (subjectId: string, name: string, glyph?: string) =>
  invoke<Subject>("create_topic", { subjectId, name, glyph });
export const updateTopic = (id: string, name: string, subjectId: string, glyph?: string) =>
  invoke<Subject>("update_topic", { id, name, subjectId, glyph });
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
export const listChunks = (sourceId: string) =>
  invoke<ChunkInfo[]>("list_chunks", { sourceId });
export const addSource = (input: AddSourceInput) =>
  invoke<IngestResult>("add_source", { input });

// ---- search / settings / demo ----
export const searchChunks = (query: string, subjectId?: string, k?: number) =>
  invoke<ChunkHit[]>("search_chunks", { query, subjectId, k });
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
  sourceId?: string
) => invoke<ChatAnswer>("chat_answer", { subjectId, level, query, sourceId });
export const generateCheatsheet = (subjectId: string, topicId?: string) =>
  invoke<CheatsheetData>("generate_cheatsheet", { subjectId, topicId });
export const getCheatsheet = (subjectId: string, topicId?: string) =>
  invoke<CheatsheetData | null>("get_cheatsheet", { subjectId, topicId });
export const generateMaterial = (
  subjectId: string,
  kind: "flashcards" | "quiz" | "audio" | "infographic" | "slideshow",
  topicId?: string,
  title?: string
) => invoke<MaterialRec>("generate_material", { subjectId, kind, topicId, title });
export const listMaterials = (subjectId: string) =>
  invoke<MaterialRec[]>("list_materials", { subjectId });

// ---- lecture recording (Whisper) ----
export const saveRecording = (
  subjectId: string,
  name: string,
  audio: number[],
  topicId?: string
) => invoke<IngestResult>("save_recording", { subjectId, name, audio, topicId });

// Near-live transcription: transcribe an audio slice and return its text (or ""
// if no Whisper transcriber is installed). Used by the recorder's live panel.
export const transcribePartial = (audio: number[]) =>
  invoke<string>("transcribe_partial", { audio });

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

// ---- web search (SearXNG on the user's homelab) ----
export interface WebResult {
  title: string;
  url: string;
  host: string;
  snippet: string;
  engine: string;
}
export const webSearch = (query: string, categories?: string) =>
  invoke<WebResult[]>("web_search", { query, categories });

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

// ---- events ----
export const onIngestProgress = (
  cb: (p: IngestProgress) => void
): Promise<UnlistenFn> => listen<IngestProgress>("ingest:progress", (e) => cb(e.payload));
