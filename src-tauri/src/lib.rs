//! Cortex backend library. Wires the SQLite-backed AppState into Tauri and
//! registers the command surface consumed by the Svelte frontend.

mod anki;
mod calendar;
mod commands;
mod db;
mod embed;
mod error;
mod google;
mod ingest;
mod llm;
mod models;
mod notes;
mod repo;
mod review;
mod vector;

use db::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Per-app data dir (created if missing); DB lives at cortex.db.
            let dir = app
                .path()
                .app_data_dir()
                .expect("resolve app data dir");
            std::fs::create_dir_all(&dir).expect("create app data dir");
            let db_path = dir.join("cortex.db");
            let state = AppState::new(&db_path).expect("init database");
            app.manage(state);

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
                    });
                }
            }

            Ok(())
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
            commands::list_sources,
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
            commands::get_cheatsheet,
            commands::export_pdf,
            commands::export_database,
            commands::export_anki,
            commands::optimize_db,
            commands::generate_material,
            commands::list_materials,
            commands::delete_material,
            commands::rename_material,
            commands::get_all_settings,
            commands::set_settings,
            commands::save_recording,
            commands::transcribe_partial,
            commands::web_search,
            commands::add_memory,
            commands::list_memory,
            commands::delete_memory,
            commands::db_stats,
            commands::delete_all_data,
            commands::ping_url,
            commands::fetch_page,
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
            calendar::delete_event,
            calendar::set_event_done,
            calendar::check_reminders,
            // review
            review::record_attempt,
            review::review_set,
            review::srs_grade,
            review::srs_due,
            review::srs_stats,
            // google calendar
            google::google_status,
            google::google_connect,
            google::google_disconnect,
            google::google_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cortex");
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
        let tid = repo::insert_topic(&c, &sid, "Dynamic programming", None).unwrap();
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
