const COMMANDS: &[&str] = &[
    "start_recording",
    "stop_recording",
    "pause_recording",
    "resume_recording",
    "recording_state",
    "read_recording_bytes",
    "list_inbox",
    "delete_recording",
    "set_widget_snapshot",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
