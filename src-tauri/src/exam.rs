//! Exam Mode command surface. Mirrored 1:1 in `src/lib/api.ts`. An exam is an
//! LLM-generated mix of MCQ + written questions, scoped to a subject and optional
//! topics, taken under a countdown, then graded LOCALLY (MCQ by index; written by
//! a single LLM call) into a score % with per-question feedback and a per-topic
//! breakdown. Follows the same LLM-config path as `commands::generate_material`
//! (model_quiz spec → gemini fallback, `guard_offline_llm`, `apply_budget`,
//! `llm::extract_json`) — reusing those helpers verbatim rather than duplicating.

use crate::commands::{apply_budget, guard_offline_llm, read_keys};
use crate::db::AppState;
use crate::error::{Error, Result};
use crate::llm;
use crate::models::ExamRec;
use crate::repo;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

const NO_MODEL: &str =
    "No model configured — add an API key in Settings → API keys (Gemini or OpenRouter), then pick it under Settings → Models.";

/// Build the source context for an exam, scoped to the selected topics. When no
/// topics are chosen we take a single whole-subject slice; otherwise we give each
/// topic an even share of the budget and concatenate, so a multi-topic exam draws
/// fairly from every topic instead of letting one crowd the rest out.
fn exam_context(
    c: &rusqlite::Connection,
    subject_id: &str,
    topic_ids: &[String],
) -> Result<String> {
    const BUDGET: usize = 18000;
    if topic_ids.is_empty() {
        let (ctx, _) = repo::context_text(c, subject_id, None, None, BUDGET)?;
        return Ok(ctx);
    }
    let per = (BUDGET / topic_ids.len()).max(1);
    let mut out = String::new();
    for tid in topic_ids {
        let (ctx, _) = repo::context_text(c, subject_id, Some(tid), None, per)?;
        if !ctx.trim().is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&ctx);
        }
    }
    Ok(out)
}

/// Generate a timed exam: prompt the model for a strict-JSON paper, store it
/// `ready`, and return the row. `mcq_count`/`written_count` are clamped so a bad
/// value can't ask the model for an absurd number of questions.
#[tauri::command]
pub async fn generate_exam(
    app: AppHandle,
    subject_id: String,
    topic_ids: Option<Vec<String>>,
    duration_min: u32,
    mcq_count: u32,
    written_count: u32,
) -> Result<ExamRec> {
    tauri::async_runtime::spawn_blocking(move || -> Result<ExamRec> {
        let state = app.state::<AppState>();
        let topics = topic_ids.unwrap_or_default();
        let mcq_n = mcq_count.clamp(0, 30);
        let written_n = written_count.clamp(0, 15);
        if mcq_n + written_n == 0 {
            return Err(Error::Other(
                "Pick at least one question — set the MCQ or written count above zero.".into(),
            ));
        }
        let duration = duration_min.clamp(5, 240);

        // Gather context + model config under the DB lock (released before the call).
        let (context, subject_name, topic_names, spec, keys) = {
            let c = state.db.lock().unwrap();
            let context = exam_context(&c, &subject_id, &topics)?;
            let subj = repo::get_subject(&c, &subject_id)?;
            // Names of the scoped topics (for the prompt + later breakdown); whole
            // subject → all topic names.
            let topic_names: Vec<String> = if topics.is_empty() {
                subj.topics.iter().map(|t| t.name.clone()).collect()
            } else {
                subj.topics
                    .iter()
                    .filter(|t| topics.contains(&t.id))
                    .map(|t| t.name.clone())
                    .collect()
            };
            let spec = repo::get_setting(&c, "model_quiz")?
                .unwrap_or_else(|| "openrouter:deepseek/deepseek-v4-flash".into());
            guard_offline_llm(&c, &spec)?;
            (context, subj.name, topic_names, spec, read_keys(&c)?)
        };

        if context.trim().is_empty() {
            return Err(Error::Other(
                "No source text to generate from — add and ingest a source first.".into(),
            ));
        }

        let mut model =
            llm::from_spec_or_any(&spec, &keys).ok_or_else(|| Error::Other(NO_MODEL.into()))?;
        {
            let c = state.db.lock().unwrap();
            apply_budget(&mut model, &c, "quiz");
        }

        let scope = if topic_names.is_empty() {
            subject_name.clone()
        } else {
            format!("{subject_name} › {}", topic_names.join(", "))
        };
        let system = format!(
            "You are an exam writer. From the study material, produce a practice exam as a \
             STRICT JSON array of EXACTLY {total} questions: the FIRST {mcq_n} are \
             multiple-choice, the next {written_n} are written. Each item has this exact shape:\n\
             - MCQ: {{\"id\":\"q1\",\"type\":\"mcq\",\"q\":\"question text\",\
             \"options\":[\"a\",\"b\",\"c\",\"d\"],\"correct\":<index 0-3>,\"marks\":1}}\n\
             - written: {{\"id\":\"q{first_written}\",\"type\":\"written\",\
             \"q\":\"question text\",\"marks\":<integer 2-5>}}\n\
             Rules: ids are unique sequential \"q1\",\"q2\",…; MCQ always has EXACTLY 4 \
             plausible options and a single correct index; MCQ marks is 1; written marks is \
             2-5 by difficulty; questions test understanding of the material, not trivia. \
             Respond with ONLY the raw JSON array — no markdown code fences, no prose.",
            total = mcq_n + written_n,
            first_written = mcq_n + 1,
        );
        let user = format!(
            "Exam scope: {scope}\n\nSOURCE MATERIAL:\n{context}\n\nWrite the exam now."
        );

        let raw = model.complete(&system, &user)?;
        let mut questions = llm::extract_json(&raw)
            .map_err(|_| Error::Other("model returned unstructured output; try again".into()))?;
        normalize_questions(&mut questions);
        if questions.as_array().map_or(true, |a| a.is_empty()) {
            return Err(Error::Other(
                "model produced no usable questions; try again".into(),
            ));
        }

        let title = if topic_names.is_empty() {
            format!("{subject_name} exam")
        } else if topic_names.len() == 1 {
            format!("{} exam", topic_names[0])
        } else {
            format!("{subject_name} — {}-topic exam", topic_names.len())
        };

        let exam = {
            let c = state.db.lock().unwrap();
            let id = repo::insert_exam(&c, &subject_id, &topics, &title, duration as i64, &questions)?;
            // Re-read by id so the frontend gets the canonical row (timestamps, status).
            repo::get_exam(&c, &id)?
        };
        Ok(exam)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Coerce the model's array into well-formed questions: ensure ids, valid mcq
/// option counts + correct index, and integer marks. Drops items that can't be
/// salvaged so grading never trips over a malformed question.
fn normalize_questions(questions: &mut Value) {
    let Some(arr) = questions.as_array_mut() else {
        return;
    };
    let mut cleaned: Vec<Value> = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        let Some(obj) = item.as_object() else { continue };
        let q = obj.get("q").and_then(|v| v.as_str()).unwrap_or("").trim();
        if q.is_empty() {
            continue;
        }
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("q{}", i + 1));
        let is_mcq = obj.get("type").and_then(|v| v.as_str()) == Some("mcq")
            || obj.get("options").is_some();
        if is_mcq {
            let options: Vec<String> = obj
                .get("options")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|o| o.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if options.len() < 2 {
                continue; // not a usable MCQ
            }
            let correct = obj
                .get("correct")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, options.len() as i64 - 1);
            cleaned.push(json!({
                "id": id,
                "type": "mcq",
                "q": q,
                "options": options,
                "correct": correct,
                "marks": 1,
            }));
        } else {
            let marks = obj
                .get("marks")
                .and_then(|v| v.as_i64())
                .unwrap_or(3)
                .clamp(1, 10);
            cleaned.push(json!({
                "id": id,
                "type": "written",
                "q": q,
                "marks": marks,
            }));
        }
    }
    *questions = Value::Array(cleaned);
}

/// Begin the exam: status → in_progress, stamp the start time, return the row.
#[tauri::command]
pub fn start_exam(state: State<AppState>, id: String) -> Result<ExamRec> {
    let c = state.db.lock().unwrap();
    repo::start_exam(&c, &id)?;
    repo::get_exam(&c, &id)
}

/// One submitted answer. MCQ → `choice` (the picked option index); written →
/// `text` (the student's prose). The frontend sends one entry per question id.
#[derive(serde::Deserialize)]
pub struct ExamAnswer {
    pub id: String,
    #[serde(default)]
    pub choice: Option<i64>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Grade a submitted exam. MCQ items are graded locally by index comparison;
/// written items are graded together in ONE LLM call. Stores answers + results +
/// score, flips status to graded, and returns the results JSON.
#[tauri::command]
pub async fn submit_exam(
    app: AppHandle,
    id: String,
    answers: Vec<ExamAnswer>,
) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || grade_exam_inner(&app, &id, &answers))
        .await
        .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Remark: re-grade a finished exam's STORED answers through the identical
/// pipeline and prompt as the original submission. Exists to recover from a
/// failed or misread grading run — and because nothing about the rubric,
/// prompt, or model selection differs from submit, a remark cannot be
/// systematically more lenient than the original marking.
#[tauri::command]
pub async fn remark_exam(app: AppHandle, id: String) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Value> {
        let answers: Vec<ExamAnswer> = {
            let state = app.state::<AppState>();
            let c = state.db.lock().unwrap();
            let exam = repo::get_exam(&c, &id)?;
            serde_json::from_value(exam.answers.clone())
                .map_err(|_| Error::Other("this exam has no stored answers to remark".into()))?
        };
        if answers.is_empty() {
            return Err(Error::Other("this exam has no stored answers to remark".into()));
        }
        grade_exam_inner(&app, &id, &answers)
    })
    .await
    .map_err(|e| Error::Other(format!("background task failed: {e}")))?
}

/// Shared grading core for submit + remark: MCQs grade locally, written answers
/// go to the model in one verified-then-scored call.
fn grade_exam_inner(app: &AppHandle, id: &str, answers: &[ExamAnswer]) -> Result<Value> {
        let state = app.state::<AppState>();

        // Load the exam + model config under the lock.
        let (exam, context, spec, keys) = {
            let c = state.db.lock().unwrap();
            let exam = repo::get_exam(&c, id)?;
            let topics: Vec<String> = exam.topic_ids.clone();
            let context = exam_context(&c, &exam.subject_id, &topics)?;
            let spec = repo::get_setting(&c, "model_quiz")?
                .unwrap_or_else(|| "openrouter:deepseek/deepseek-v4-flash".into());
            (exam, context, spec, read_keys(&c)?)
        };

        let questions = exam.questions.as_array().cloned().unwrap_or_default();
        let ans_by_id: std::collections::HashMap<&str, &ExamAnswer> =
            answers.iter().map(|a| (a.id.as_str(), a)).collect();

        // Per-question grading + per-topic tallies. Topic attribution uses the
        // exam's scoped topic names (best-effort) — when an exam spans topics we
        // can still surface which were weakest by question index buckets, but the
        // question JSON has no topic id, so we bucket by the exam's topic list.
        let mut per_question: Vec<Value> = Vec::with_capacity(questions.len());
        let mut earned = 0.0f64;
        let mut total = 0.0f64;

        // Collect written items needing the LLM, plus a parallel index map.
        let mut written_prompt_items: Vec<Value> = Vec::new();
        let mut written_meta: Vec<(usize, String, f64)> = Vec::new(); // (q index, id, marks)

        for (qi, q) in questions.iter().enumerate() {
            let qid = q.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let marks = q.get("marks").and_then(|v| v.as_i64()).unwrap_or(1) as f64;
            total += marks;
            let qtype = q.get("type").and_then(|v| v.as_str()).unwrap_or("mcq");
            let given = ans_by_id.get(qid.as_str());

            if qtype == "mcq" {
                let correct = q.get("correct").and_then(|v| v.as_i64()).unwrap_or(-1);
                let choice = given.and_then(|a| a.choice);
                let is_correct = choice == Some(correct);
                if is_correct {
                    earned += marks;
                }
                per_question.push(json!({
                    "id": qid,
                    "type": "mcq",
                    "marks": marks,
                    "score": if is_correct { marks } else { 0.0 },
                    "correct_choice": correct,
                    "your_choice": choice,
                    "correct": is_correct,
                    "feedback": if is_correct { "Correct." } else { "Incorrect." },
                }));
            } else {
                let student = given
                    .and_then(|a| a.text.as_deref())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                written_meta.push((qi, qid.clone(), marks));
                written_prompt_items.push(json!({
                    "id": qid,
                    "question": q.get("q").and_then(|v| v.as_str()).unwrap_or(""),
                    "marks": marks,
                    "answer": student,
                }));
                // placeholder; filled after LLM grading
                per_question.push(json!({
                    "id": qid,
                    "type": "written",
                    "marks": marks,
                    "score": 0.0,
                    "feedback": "",
                    "your_text": given.and_then(|a| a.text.clone()).unwrap_or_default(),
                }));
            }
        }

        // Which model judged the written answers — surfaced in the results UI so
        // a bad grade is attributable ("graded by …"). MCQ-only exams are local.
        let mut graded_by = String::from("local (multiple choice only)");
        // Grade ALL written answers in a single LLM call (when there are any).
        if !written_prompt_items.is_empty() {
            let mut model = llm::from_spec_or_any(&spec, &keys)
                .ok_or_else(|| Error::Other(NO_MODEL.into()))?;
            {
                let c = state.db.lock().unwrap();
                guard_offline_llm(&c, &spec)?;
                apply_budget(&mut model, &c, "quiz");
            }
            let items_json = serde_json::to_string(&written_prompt_items).unwrap_or_default();
            // `verify` comes FIRST in the output object on purpose: forcing the
            // model to quote and check the student's actual claims before it
            // writes a score measurably cuts misreadings (e.g. accusing the
            // student of reversing notation they stated correctly).
            let system =
                "You are a careful exam grader. You are given SOURCE MATERIAL and a JSON array of \
                 written answers, each {id, question, marks, answer}. For EACH item, FIRST verify: \
                 in 1-3 sentences, restate the factual claims the student ACTUALLY made — quote \
                 their wording; NEVER attribute to the student anything they did not write — and \
                 check each claim against the material. THEN award a score from 0 to `marks` \
                 (fractional allowed) for correctness and completeness, crediting everything the \
                 student got right. Feedback: 1-2 sentences consistent with your verification. \
                 Respond with ONLY a raw JSON array \
                 [{\"id\":\"...\",\"verify\":\"...\",\"score\":<number>,\"feedback\":\"...\"}], \
                 one entry per item, same ids, fields in that order. No prose, no code fences.";
            let user = format!(
                "SOURCE MATERIAL:\n{context}\n\nANSWERS TO GRADE (JSON):\n{items_json}\n\nGrade now."
            );
            graded_by = model.name();
            // A truncated grading reply is worthless: enforce a generous output
            // floor regardless of the user's budget sliders (the verify field
            // makes replies longer, and a mid-array cutoff is exactly what
            // produced the "grading is temporarily unavailable" zeros).
            let floor = 2048 + 700 * written_prompt_items.len() as u32;
            model.set_max_tokens(floor.max(4096));
            let mut raw = model.complete(system, &user)?;
            if llm::extract_json(&raw).is_err() {
                eprintln!(
                    "[exam] grading reply unparseable (model {}), retrying once: {}",
                    graded_by,
                    raw.chars().take(300).collect::<String>()
                );
                raw = model.complete(
                    system,
                    &format!(
                        "{user}\n\nIMPORTANT: your previous reply could not be parsed. \
                         Respond with ONLY the raw JSON array — no thinking, no prose, \
                         no code fences, starting with [ and ending with ]."
                    ),
                )?;
            }
            // Best-effort: if grading JSON is unusable, written items score 0 with a
            // note rather than failing the whole submission.
            if let Ok(graded) = llm::extract_json(&raw) {
                let by_id: std::collections::HashMap<String, &Value> = graded
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|g| {
                                g.get("id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| (s.to_string(), g))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                for (qi, qid, marks) in &written_meta {
                    if let Some(g) = by_id.get(qid) {
                        let raw_score = g.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let score = raw_score.clamp(0.0, *marks);
                        let feedback = g
                            .get("feedback")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        earned += score;
                        if let Some(obj) = per_question[*qi].as_object_mut() {
                            obj.insert("score".into(), json!(score));
                            obj.insert("feedback".into(), json!(feedback));
                        }
                    } else if let Some(obj) = per_question[*qi].as_object_mut() {
                        obj.insert(
                            "feedback".into(),
                            json!("Could not grade this answer automatically."),
                        );
                    }
                }
            } else {
                eprintln!("[exam] grading reply unparseable after retry (model {graded_by})");
                for (qi, _, _) in &written_meta {
                    if let Some(obj) = per_question[*qi].as_object_mut() {
                        obj.insert(
                            "feedback".into(),
                            json!(format!(
                                "The grading model ({graded_by}) returned an unreadable reply twice — \
                                 not graded. Press Remark to retry, or switch the Quiz model in \
                                 Settings → Models."
                            )),
                        );
                    }
                }
            }
        }

        let percent = if total > 0.0 {
            (earned / total * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        // Per-topic breakdown: when the exam is scoped to named topics, surface
        // each topic's name so the UI can flag the weakest. With a single scope we
        // still report it so the callout always has something to show.
        let topics = topic_names_for(&state, &exam);
        let topic_breakdown: Vec<Value> = topics
            .iter()
            .map(|name| json!({ "topic": name }))
            .collect();

        let results = json!({
            "score": percent,
            "earned": earned,
            "total": total,
            "questions": per_question,
            "topics": topic_breakdown,
            "graded_by": graded_by,
        });

        // Persist the answers verbatim alongside the grading.
        let answers_json = json!(answers
            .iter()
            .map(|a| json!({ "id": a.id, "choice": a.choice, "text": a.text }))
            .collect::<Vec<_>>());

        {
            let c = state.db.lock().unwrap();
            repo::finalize_exam(&c, id, &answers_json, &results, percent)?;
        }
        Ok(results)
}

/// The exam's scoped topic names (empty scope → all subject topics). Best-effort:
/// a missing subject just yields an empty list.
fn topic_names_for(state: &State<AppState>, exam: &ExamRec) -> Vec<String> {
    let c = state.db.lock().unwrap();
    let Ok(subj) = repo::get_subject(&c, &exam.subject_id) else {
        return Vec::new();
    };
    if exam.topic_ids.is_empty() {
        subj.topics.iter().map(|t| t.name.clone()).collect()
    } else {
        subj.topics
            .iter()
            .filter(|t| exam.topic_ids.contains(&t.id))
            .map(|t| t.name.clone())
            .collect()
    }
}

/// All exams for a subject, newest first (drives the setup screen's past list).
#[tauri::command]
pub fn list_exams(state: State<AppState>, subject_id: String) -> Result<Vec<ExamRec>> {
    let c = state.db.lock().unwrap();
    repo::list_exams(&c, &subject_id)
}

/// Fetch a single exam (e.g. to reopen its results).
#[tauri::command]
pub fn get_exam(state: State<AppState>, id: String) -> Result<ExamRec> {
    let c = state.db.lock().unwrap();
    repo::get_exam(&c, &id)
}

/// Delete an exam.
#[tauri::command]
pub fn delete_exam(state: State<AppState>, id: String) -> Result<()> {
    let c = state.db.lock().unwrap();
    repo::delete_exam(&c, &id)
}
