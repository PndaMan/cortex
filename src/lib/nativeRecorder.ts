// JS bridge to the native iOS recorder + widget plugin (`tauri-plugin-cortex-ios`).
//
// Everything here is a no-op / throws off-iOS — callers gate on `nativeRecorderAvailable`.
// The native recorder lets a lecture keep recording when the app is backgrounded or the screen
// is locked (AVAudioSession + the `audio` background mode), and powers the Live Activity + the
// Home/Lock-screen record button. The existing Recorder UI is unchanged — it just drives this
// instead of WKWebView `getUserMedia` on iOS, and feeds the captured bytes through the SAME
// `saveRecording` pipeline.

import { invoke, addPluginListener, type PluginListener } from "@tauri-apps/api/core";
import { isIOS } from "./platform";

const PLUGIN = "cortex-ios";

export const nativeRecorderAvailable = isIOS;

export interface RecordingPath {
  path: string;
  durationSec: number;
}
export interface RecordingStatus {
  isRecording: boolean;
  isPaused: boolean;
  elapsed: number;
  fileName?: string;
}
export interface InboxFile {
  path: string;
  name: string;
  subject?: string;
  size: number;
}
export interface Tick {
  level: number;
  elapsed: number;
}
export interface StateEvt {
  isRecording: boolean;
  isPaused: boolean;
  elapsed: number;
  finishedFile?: string;
}

export async function startNative(subject?: string, accent?: string): Promise<RecordingPath> {
  return invoke<RecordingPath>(`plugin:${PLUGIN}|start_recording`, { args: { subject, accent } });
}
export async function stopNative(): Promise<RecordingPath> {
  return invoke<RecordingPath>(`plugin:${PLUGIN}|stop_recording`);
}
export async function pauseNative(): Promise<void> {
  await invoke(`plugin:${PLUGIN}|pause_recording`);
}
export async function resumeNative(): Promise<void> {
  await invoke(`plugin:${PLUGIN}|resume_recording`);
}
export async function recordingState(): Promise<RecordingStatus> {
  return invoke<RecordingStatus>(`plugin:${PLUGIN}|recording_state`);
}
export async function readRecordingBytes(path: string): Promise<number[]> {
  const r = await invoke<{ bytes: number[] }>(`plugin:${PLUGIN}|read_recording_bytes`, { args: { path } });
  return r.bytes ?? [];
}
export async function listInbox(): Promise<InboxFile[]> {
  const r = await invoke<{ files: InboxFile[] }>(`plugin:${PLUGIN}|list_inbox`);
  return r.files ?? [];
}
export async function deleteRecording(path: string): Promise<void> {
  await invoke(`plugin:${PLUGIN}|delete_recording`, { args: { path } });
}
export async function setWidgetSnapshot(json: string): Promise<void> {
  await invoke(`plugin:${PLUGIN}|set_widget_snapshot`, { args: { json } });
}

export interface MicPermission { granted: boolean; status: string }
/** "granted" | "denied" | "undetermined" — does NOT prompt. */
export async function micPermissionStatus(): Promise<string> {
  const r = await invoke<{ status: string }>(`plugin:${PLUGIN}|mic_permission_status`);
  return r.status ?? "undetermined";
}
/** Request mic permission; shows the system prompt when undetermined. */
export async function requestMicPermission(): Promise<MicPermission> {
  return invoke<MicPermission>(`plugin:${PLUGIN}|request_mic_permission`);
}
/** Open this app's page in Settings (to enable Microphone after a denial). */
export async function openAppSettings(): Promise<void> {
  await invoke(`plugin:${PLUGIN}|open_app_settings`);
}

export async function onTick(cb: (t: Tick) => void): Promise<PluginListener> {
  return addPluginListener(PLUGIN, "tick", (e: unknown) => cb(e as Tick));
}
export async function onState(cb: (s: StateEvt) => void): Promise<PluginListener> {
  return addPluginListener(PLUGIN, "state", (e: unknown) => cb(e as StateEvt));
}
