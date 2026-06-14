//! Cortex backend library. Wires the SQLite-backed AppState into Tauri and
//! registers the command surface consumed by the Svelte frontend.

mod analytics;
mod anki;
mod backup;
mod calendar;
mod commands;
mod db;
mod embed;
mod error;
mod exam;
mod google;
mod homelab;
mod ingest;
mod llm;
mod models;
mod moodle;
mod mpv;
mod notes;
mod repo;
mod review;
mod sync;
mod vector;

use db::AppState;
use tauri::Manager;

/// Reveal (and focus) the main window — used by the tray's Open and left-click.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single instance MUST be the first plugin. A second launch (e.g. from
        // the app menu while Cortex is already running) just reveals/focuses the
        // existing window instead of spawning another process.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Moodle SSO callback: the launch flow redirects to cortexmoodle://token=…
        // This handler receives the RAW callback URI (so the base64 token isn't
        // corrupted by URL normalization) and hands it to the moodle module.
        .register_uri_scheme_protocol("cortexmoodle", |ctx, request| {
            let raw = request.uri().to_string();
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || moodle::handle_sso_uri(&app, &raw));
            tauri::http::Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(
                    b"<html><body style=\"font-family:system-ui;padding:2rem;background:#111;color:#eee\">Signed in to Moodle. You can close this window.</body></html>"
                        .to_vec(),
                )
                .unwrap()
        })
        .setup(|app| {
            // Per-app data dir (created if missing); DB lives at cortex.db.
            let dir = app
                .path()
                .app_data_dir()
                .expect("resolve app data dir");
            std::fs::create_dir_all(&dir).expect("create app data dir");
            let db_path = dir.join("cortex.db");
            // NOTE: the homelab pull/merge intentionally does NOT run here — it
            // would block startup on network I/O (up to the HTTP timeout if the
            // homelab is unreachable). The frontend calls `sync_pull` in the
            // background after the window is shown; see store loadSyncStatus.
            let state = AppState::new(&db_path).expect("init database");
            app.manage(state);

            // If a previous session crashed, its mpv music sidecar is still
            // alive and streaming — quit it before this session starts its own.
            mpv::cleanup_stale(app.handle());

            // Tray icon: lets the app keep working (ingest, generation, music)
            // after the window is closed. Left-click or "Open" reopens it.
            {
                use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                let open = MenuItem::with_id(app, "open", "Open Cortex", true, None::<&str>)?;
                let dashboard =
                    MenuItem::with_id(app, "dashboard", "Go to Dashboard", true, None::<&str>)?;
                let music =
                    MenuItem::with_id(app, "music", "Play / pause music", true, None::<&str>)?;
                let sep1 = PredefinedMenuItem::separator(app)?;
                let restart = MenuItem::with_id(app, "restart", "Restart Cortex", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit Cortex", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&open, &dashboard, &music, &sep1, &restart, &quit])?;
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
                TrayIconBuilder::with_id("cortex-tray")
                    .icon(icon)
                    .tooltip("Cortex")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "open" => show_main_window(app),
                        "dashboard" => {
                            use tauri::Emitter;
                            show_main_window(app);
                            let _ = app.emit("tray-go-dashboard", ());
                        }
                        "music" => {
                            use tauri::Emitter;
                            let _ = app.emit("tray-music-toggle", ());
                        }
                        "restart" => app.restart(),
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            show_main_window(tray.app_handle());
                        }
                    })
                    .build(app)?;
            }

            // On Linux (WebKitGTK) getUserMedia is denied by default, which
            // surfaces as NotAllowedError in the recorder. Auto-grant audio
            // media permission requests for the app's own webview.
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.with_webview(|webview| {
                        use webkit2gtk::glib::Cast;
                        use webkit2gtk::{
                            PermissionRequestExt, UserMediaPermissionRequest, WebViewExt,
                        };
                        let wv = webview.inner();
                        wv.connect_permission_request(|_wv, req| {
                            if req.downcast_ref::<UserMediaPermissionRequest>().is_some() {
                                req.allow();
                                true
                            } else {
                                false
                            }
                        });

                        // WebKitGTK's built-in find-in-page grabs Ctrl/Ctrl+F at the
                        // GTK level — BELOW the webview's JS — so a JS capture-phase
                        // handler can't stop it, and it steals Ctrl+C/Ctrl+V (the
                        // user can't copy/paste). Intercept Ctrl+F here: block the
                        // native handler and open OUR find bar via a synthetic event.
                        // All other keys (incl. copy/paste) pass straight through.
                        use gtk::prelude::WidgetExt;
                        wv.connect_key_press_event(|wv: &webkit2gtk::WebView, ev: &gdk::EventKey| {
                            let ctrl = ev.state().contains(gdk::ModifierType::CONTROL_MASK);
                            let key = ev.keyval().name().map(|s| s.to_string()).unwrap_or_default();
                            if ctrl && (key == "f" || key == "F") {
                                wv.run_javascript(
                                    "window.dispatchEvent(new KeyboardEvent('keydown',{key:'f',ctrlKey:true,bubbles:true}))",
                                    None::<&webkit2gtk::gio::Cancellable>,
                                    |_| {},
                                );
                                return gtk::glib::Propagation::Stop;
                            }
                            gtk::glib::Propagation::Proceed
                        });
                    });
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Close-to-tray (Settings → "close to tray", default ON): hide the
            // window instead of exiting so ingest/generation/music continue.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = {
                    let state = window.state::<AppState>();
                    let c = state.db.lock().unwrap();
                    repo::get_setting(&c, "close_to_tray")
                        .ok()
                        .flatten()
                        .as_deref()
                        != Some("false")
                };
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_subjects,
            commands::get_subject,
            commands::create_subject,
            commands::update_subject,
            commands::delete_subject,
            commands::create_topic,
            commands::update_topic,
            commands::delete_topic,
            commands::reorder_subjects,
            commands::reorder_topics,
            commands::list_sources,
            commands::list_failed_sources,
            commands::get_source,
            commands::update_source,
            commands::delete_source,
            commands::move_source,
            commands::list_chunks,
            commands::get_setting,
            commands::set_setting,
            commands::add_source,
            commands::reingest_source,
            commands::search_chunks,
            commands::global_search,
            commands::seed_demo,
            commands::env_probe,
            commands::chat_answer,
            commands::add_chat_message,
            commands::list_chat_messages,
            commands::clear_chat,
            commands::new_chat,
            commands::list_chat_threads,
            commands::open_chat_thread,
            commands::generate_cheatsheet,
            commands::generate_subject_cheatsheet,
            commands::get_cheatsheet,
            commands::get_subject_cheatsheet,
            commands::update_cheatsheet,
            commands::list_cheatsheet_versions,
            commands::get_cheatsheet_version,
            commands::restore_cheatsheet_version,
            commands::export_pdf,
            commands::export_database,
            commands::export_anki,
            commands::import_anki,
            backup::backup_status,
            backup::backup_now,
            commands::optimize_db,
            commands::generate_material,
            commands::synthesize_overview,
            commands::list_materials,
            commands::delete_material,
            commands::rename_material,
            commands::add_citation,
            commands::list_citations,
            commands::update_citation,
            commands::delete_citation,
            commands::get_all_settings,
            commands::set_settings,
            commands::save_recording,
            commands::transcribe_partial,
            commands::web_search,
            commands::add_memory,
            commands::list_memory,
            commands::delete_memory,
            // custom music stations + YouTube-audio (mpv sidecar)
            commands::list_custom_stations,
            commands::add_custom_station,
            commands::delete_custom_station,
            commands::reorder_custom_stations,
            mpv::media_tools_status,
            mpv::youtube_play,
            mpv::youtube_pause,
            mpv::youtube_resume,
            mpv::youtube_stop,
            mpv::youtube_set_volume,
            mpv::youtube_prewarm,
            commands::db_stats,
            commands::delete_all_data,
            commands::ping_url,
            commands::omarchy_theme,
            commands::fetch_page,
            sync::sync_status,
            sync::sync_test,
            sync::sync_push,
            sync::sync_pull,
            moodle::moodle_connect,
            moodle::moodle_set_token,
            moodle::moodle_login_sso,
            moodle::moodle_status,
            moodle::moodle_disconnect,
            moodle::moodle_sync,
            moodle::moodle_data,
            moodle::moodle_link_subject,
            moodle::moodle_autolink,
            commands::set_subject_framework,
            commands::get_subject_framework,
            commands::get_subject_framework_text,
            commands::clear_subject_framework,
            commands::set_subject_aliases,
            commands::retag_calendar_events,
            commands::dependency_status,
            // notes
            notes::create_note,
            notes::list_notes,
            notes::get_note,
            notes::update_note,
            notes::delete_note,
            notes::note_to_source,
            // calendar
            calendar::create_event,
            calendar::list_events,
            calendar::update_event,
            calendar::set_event_checklist,
            calendar::delete_event,
            calendar::set_event_done,
            calendar::check_reminders,
            // review
            review::record_attempt,
            review::review_set,
            review::srs_grade,
            review::srs_due,
            review::srs_preview,
            review::srs_stats,
            // study analytics
            analytics::log_pomodoro_session,
            analytics::analytics_summary,
            // exam mode
            exam::generate_exam,
            exam::start_exam,
            exam::submit_exam,
            exam::remark_exam,
            exam::list_exams,
            exam::get_exam,
            exam::delete_exam,
            // google calendar
            google::google_status,
            google::google_connect,
            google::google_disconnect,
            google::google_sync,
            google::google_list_calendars,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Cortex")
        .run(|_app, event| {
            // Tear down the headless mpv music sidecar when the app exits.
            if let tauri::RunEvent::Exit = event {
                mpv::shutdown();
            }
        });
}

#[cfg(test)]
mod pipeline_tests {
    //! End-to-end offline proof of the ingestion + retrieval data flow, exercised
    //! without the GUI: parse → chunk → embed(stub) → store → cosine search.
    use crate::embed::Embedder;
    use crate::{db::AppState, embed, ingest, repo, vector::f32s_to_blob};

    #[test]
    fn ingest_then_search_finds_the_source() {
        let st = AppState::in_memory().unwrap();
        let c = st.db.lock().unwrap();

        let sid = repo::insert_subject(&c, "Algorithms", Some("CS-3490"), None, None).unwrap();
        let tid = repo::insert_topic(&c, &sid, "Dynamic programming", None, &[]).unwrap();
        let srcid = repo::insert_source(&c, &sid, Some(&tid), "dp.md", "md", None).unwrap();

        let text = "Dynamic programming solves problems by combining solutions to \
                    overlapping subproblems. Memoization caches results top-down; \
                    tabulation fills a table bottom-up. The coin change recurrence is \
                    dp[a] = 1 + min over coins c of dp[a - c].";
        let chunks = ingest::chunk_text(text, 120, 30);
        assert!(!chunks.is_empty(), "chunker produced chunks");

        let emb = embed::StubEmbedder;
        let vecs = ingest::embed_chunks(&emb, &chunks).unwrap();
        for (i, (chunk, v)) in chunks.iter().zip(vecs.iter()).enumerate() {
            repo::insert_chunk(
                &c, &srcid, &sid, Some(&tid), i as i64, chunk, None,
                v.len() as i64, &f32s_to_blob(v),
            )
            .unwrap();
        }
        repo::finalize_source(&c, &srcid, "ready", Some("test"), Some(text), None).unwrap();
        assert!(repo::count_chunks(&c, &srcid).unwrap() > 0, "chunks stored");

        // query retrieval: a related query should surface this source as top hit
        let qvec = &emb.embed(&["memoization overlapping subproblems".into()]).unwrap()[0];
        let hits = repo::search_chunks(&c, Some(&sid), qvec, 5).unwrap();
        assert!(!hits.is_empty(), "search returned hits");
        assert_eq!(hits[0].source_id, srcid, "top hit is the ingested source");
        assert!(hits[0].score > 0.0, "positive similarity");

        // seeded-demo style listing renders the tree end to end
        let subs = repo::list_subjects(&c).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].source_count, 1);
        assert_eq!(subs[0].topics[0].sources[0].name, "dp.md");
    }
}
