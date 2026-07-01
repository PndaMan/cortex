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
    "mic_permission_status",
    "request_mic_permission",
    "open_app_settings",
];

fn main() {
    let mut builder = tauri_plugin::Builder::new(COMMANDS);
    // Only wire the iOS Swift package when actually building FOR iOS. `.ios_path("ios")` makes
    // tauri_plugin link the Swift package (via swift-rs) into the Rust build so the
    // `@_cdecl("init_plugin_cortex_ios")` symbol that `ios_plugin_binding!` references resolves
    // (without it the iOS cdylib link fails: "_init_plugin_cortex_ios undefined"). Gated on the
    // target so the Linux/macOS/Windows desktop build never touches the iOS/Swift path.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios") {
        builder = builder.ios_path("ios");
    }
    builder.build();
}
