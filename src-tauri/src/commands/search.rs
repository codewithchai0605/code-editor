//! Workspace-wide "Find in Files" backend.
//!
//! The design goals (fast, memory-efficient, cancellable) all come from the
//! same idea: never let more than one search's worker threads be alive at
//! once, never hold more than a bounded amount of match data in memory, and
//! never make the UI wait for the whole workspace to finish before it can
//! show anything.
//!
//! - **Fast**: the walk itself uses `ignore::WalkParallel` (the same
//!   gitignore-aware parallel walker ripgrep is built on) instead of a
//!   single-threaded directory recursion, and results stream to the
//!   frontend in small batches as they're found rather than all at once at
//!   the end.
//! - **Memory-efficient**: each worker only ever holds one file's bytes at a
//!   time (dropped immediately after that file is searched), `.gitignore`
//!   rules plus a `max_filesize` cap keep huge/vendored files out of the walk
//!   entirely, and hard caps on matches-per-file/total-matched-files bound
//!   how much match data can accumulate for a single pathological search.
//! - **Cancellable**: a shared generation counter (`SearchState`) means a
//!   new search — fired 150-200ms after the last keystroke by the
//!   frontend's debounce — makes any still-running previous search's
//!   threads quit within one visited entry, so a fast typist never has two
//!   workspace walks racing each other.
use crate::commands::fs::ALWAYS_SKIP_DIRS;
use crate::state::SearchState;
use ignore::overrides::{Override, OverrideBuilder};
use ignore::{WalkBuilder, WalkState};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

// ---------------------------------------------------------------------------
// Tunables
// ---------------------------------------------------------------------------

/// Mirrors `read_text_file`'s own cap in fs.rs — anything bigger is almost
/// certainly a lockfile/bundle/binary, not something worth grep'ing, and
/// skipping it at the walk level (`max_filesize`, below) means we never even
/// stat-then-read it beyond the initial directory-entry metadata.
const MAX_SEARCHABLE_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Bytes sampled from the front of a file to decide "binary or text" (a NUL
/// byte anywhere in the sample means binary). Matches the common heuristic
/// used by git/ripgrep — cheap, and almost never wrong in practice.
const BINARY_PROBE_BYTES: usize = 8_000;

/// A line longer than this (in UTF-16 code units, matching the column units
/// below) gets windowed down to a slice around the match rather than sent
/// in full — one enormous minified/data line shouldn't balloon the memory
/// or IPC cost of a single match.
const MAX_LINE_UNITS: usize = 500;
const WINDOW_CONTEXT_BEFORE: usize = 60;

/// Per-file and whole-search caps. Beyond these, the search stops early and
/// reports `truncated: true` rather than continuing to accumulate matches
/// for what's almost always a generated/vendored file or an overly broad
/// query.
const MAX_MATCHES_PER_LINE: usize = 50;
const MAX_MATCHES_PER_FILE: usize = 200;
const MAX_TOTAL_MATCHED_FILES: usize = 1_000;
const MAX_TOTAL_MATCHES: usize = 20_000;

/// How results are batched into `search:results` events: whichever comes
/// first, a file count or a time slice, so a huge match burst doesn't flood
/// IPC with one event per file, but a slow trickle still reaches the UI
/// promptly instead of stalling until the whole batch fills up.
const BATCH_FLUSH_FILES: usize = 25;
const BATCH_FLUSH_INTERVAL: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchOptions {
    pub query: String,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub regex: bool,
    /// Comma-separated glob patterns, e.g. `"**/*.ts, morefile"`. Empty
    /// means "no restriction".
    #[serde(default)]
    pub include: String,
    #[serde(default)]
    pub exclude: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SearchError {
    InvalidRegex { message: Box<str> },
    InvalidGlob { message: Box<str> },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    /// 1-based line number.
    pub line: u32,
    /// UTF-16 code unit offsets into `line_text` — this is deliberately the
    /// same unit JS string indexing (and so CodeMirror's own positions) use,
    /// rather than raw UTF-8 byte offsets, so the frontend can place a
    /// highlight/selection directly without re-deriving an offset mapping.
    pub col_start: u32,
    pub col_end: u32,
    /// The source line the match was found on (windowed down for
    /// pathologically long lines — see `windowed_line`).
    pub line_text: Box<str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSearchResult {
    pub path: Box<str>,
    pub matches: Box<[SearchMatch]>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SearchResultBatch {
    search_id: Box<str>,
    files: Box<[FileSearchResult]>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SearchDoneEvent {
    search_id: Box<str>,
    files_matched: usize,
    matches_found: usize,
    truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceFileResult {
    pub path: Box<str>,
    pub replaced: usize,
    pub error: Option<Box<str>>,
}

// ---------------------------------------------------------------------------
// Matcher construction (shared by search + replace, so both apply the same
// case/regex/whole-word rules)
// ---------------------------------------------------------------------------

fn build_regex(options: &SearchOptions) -> Result<Regex, SearchError> {
    let core = if options.regex {
        options.query.clone()
    } else {
        regex::escape(&options.query)
    };
    let pattern = if options.whole_word {
        format!(r"\b(?:{core})\b")
    } else {
        core
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .build()
        .map_err(|e| SearchError::InvalidRegex {
            message: e.to_string().into_boxed_str(),
        })
}

/// Builds a single `Override` matcher out of the include/exclude glob lists.
/// `ignore`'s own override semantics turn out to compose both directions at
/// once: a bare glob added here is a whitelist entry (only matching paths
/// survive), while a `!`-prefixed one is always excluded regardless of the
/// whitelist — exactly the "files to include" / "files to exclude" pairing
/// this feature needs, so include and exclude can share one builder instead
/// of two independently-applied glob sets.
fn build_overrides(
    root: &Path,
    include: &str,
    exclude: &str,
) -> Result<Option<Override>, SearchError> {
    let mut builder = OverrideBuilder::new(root);
    let mut any = false;

    for pat in include.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        builder.add(pat).map_err(|e| SearchError::InvalidGlob {
            message: format!("Include pattern \"{pat}\": {e}").into_boxed_str(),
        })?;
        any = true;
    }
    for pat in exclude.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        builder
            .add(&format!("!{pat}"))
            .map_err(|e| SearchError::InvalidGlob {
                message: format!("Exclude pattern \"{pat}\": {e}").into_boxed_str(),
            })?;
        any = true;
    }

    if !any {
        return Ok(None);
    }
    let ov = builder.build().map_err(|e| SearchError::InvalidGlob {
        message: e.to_string().into_boxed_str(),
    })?;
    Ok(Some(ov))
}

// ---------------------------------------------------------------------------
// Per-file search
// ---------------------------------------------------------------------------

fn utf16_offset(s: &str, byte_offset: usize) -> u32 {
    s[..byte_offset].encode_utf16().count() as u32
}

/// Windows a line down to `MAX_LINE_UNITS` UTF-16 code units around
/// `[col_start, col_end)` when it's too long to ship in full, returning the
/// (possibly-truncated) text plus offsets re-based to that window. Below the
/// cap, the line and offsets pass through unchanged.
fn windowed_line(line: &str, col_start: u32, col_end: u32) -> (Box<str>, u32, u32) {
    let units: Vec<u16> = line.encode_utf16().collect();
    if units.len() <= MAX_LINE_UNITS {
        return (line.into(), col_start, col_end);
    }

    let start = (col_start as usize).saturating_sub(WINDOW_CONTEXT_BEFORE);
    let end = (start + MAX_LINE_UNITS)
        .max(col_end as usize)
        .min(units.len());
    let prefix = if start > 0 { "…" } else { "" };
    let suffix = if end < units.len() { "…" } else { "" };

    let windowed = String::from_utf16_lossy(&units[start..end]);
    let text = format!("{prefix}{windowed}{suffix}");
    let shift = start as i64 - prefix.encode_utf16().count() as i64;
    let new_start = (col_start as i64 - shift).max(0) as u32;
    let new_end = (col_end as i64 - shift).max(0) as u32;
    (text.into_boxed_str(), new_start, new_end)
}

/// Reads and searches one file, returning `None` for anything binary,
/// non-UTF-8, or with zero matches — all three are "nothing to report" from
/// the caller's point of view. Bytes are dropped as soon as this returns;
/// nothing from the file's content is retained beyond the matched lines
/// (and even those are capped/windowed, see above).
fn search_file(path: &Path, regex: &Regex) -> Option<Box<[SearchMatch]>> {
    let bytes = std::fs::read(path).ok()?;
    let probe = &bytes[..bytes.len().min(BINARY_PROBE_BYTES)];
    if probe.contains(&0) {
        return None; // binary
    }
    let text = std::str::from_utf8(&bytes).ok()?;

    let mut matches: Vec<SearchMatch> = Vec::new();
    'lines: for (idx, raw_line) in text.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let mut per_line = 0usize;
        for m in regex.find_iter(line) {
            if per_line >= MAX_MATCHES_PER_LINE {
                break;
            }
            let col_start = utf16_offset(line, m.start());
            let col_end = utf16_offset(line, m.end());
            let (line_text, col_start, col_end) = windowed_line(line, col_start, col_end);
            matches.push(SearchMatch {
                line: (idx + 1) as u32,
                col_start,
                col_end,
                line_text,
            });
            per_line += 1;
            if matches.len() >= MAX_MATCHES_PER_FILE {
                break 'lines;
            }
        }
    }

    if matches.is_empty() {
        None
    } else {
        Some(matches.into_boxed_slice())
    }
}

// ---------------------------------------------------------------------------
// The search itself: a parallel walk feeding a single aggregator thread,
// which is what actually talks to the frontend.
// ---------------------------------------------------------------------------

fn flush(app: &AppHandle, search_id: &str, batch: &mut Vec<FileSearchResult>) {
    if batch.is_empty() {
        return;
    }
    let files = std::mem::take(batch).into_boxed_slice();
    let _ = app.emit(
        "search:results",
        SearchResultBatch {
            search_id: search_id.into(),
            files,
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_search(
    app: AppHandle,
    search_id: String,
    root: PathBuf,
    regex: Regex,
    overrides: Option<Override>,
    generation: Arc<AtomicU64>,
    my_gen: u64,
) {
    std::thread::spawn(move || {
        let regex = Arc::new(regex);
        let (tx, rx) = mpsc::channel::<FileSearchResult>();

        let files_matched = Arc::new(AtomicU64::new(0));
        let matches_found = Arc::new(AtomicU64::new(0));
        let truncated = Arc::new(AtomicBool::new(false));

        // Aggregator: the only thread that touches `app.emit`. Runs
        // concurrently with the walk (not after it) so the panel starts
        // filling in well before a large workspace finishes scanning.
        let aggregator = {
            let app = app.clone();
            let search_id = search_id.clone();
            let files_matched = files_matched.clone();
            let matches_found = matches_found.clone();
            let truncated = truncated.clone();
            std::thread::spawn(move || {
                let mut batch = Vec::with_capacity(BATCH_FLUSH_FILES);
                let mut last_flush = Instant::now();
                loop {
                    match rx.recv_timeout(BATCH_FLUSH_INTERVAL) {
                        Ok(result) => {
                            batch.push(result);
                            if batch.len() >= BATCH_FLUSH_FILES
                                || last_flush.elapsed() >= BATCH_FLUSH_INTERVAL
                            {
                                flush(&app, &search_id, &mut batch);
                                last_flush = Instant::now();
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            flush(&app, &search_id, &mut batch);
                            last_flush = Instant::now();
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                flush(&app, &search_id, &mut batch);
                let _ = app.emit(
                    "search:done",
                    SearchDoneEvent {
                        search_id: search_id.into_boxed_str(),
                        files_matched: files_matched.load(Ordering::Relaxed) as usize,
                        matches_found: matches_found.load(Ordering::Relaxed) as usize,
                        truncated: truncated.load(Ordering::Relaxed),
                    },
                );
            })
        };

        let mut walk_builder = WalkBuilder::new(&root);
        walk_builder
            .hidden(false)
            .max_filesize(Some(MAX_SEARCHABLE_FILE_BYTES))
            .threads(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4),
            )
            .filter_entry(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| !ALWAYS_SKIP_DIRS.contains(&name))
                    .unwrap_or(true)
            });
        if let Some(ov) = overrides {
            walk_builder.overrides(ov);
        }

        // Kept alive through the whole walk purely so we can explicitly
        // `drop` it once every worker has finished — see below.
        let factory_tx = tx.clone();
        drop(tx);

        walk_builder.build_parallel().run(move || {
            let regex = regex.clone();
            let tx = factory_tx.clone();
            let generation = generation.clone();
            let files_matched = files_matched.clone();
            let matches_found = matches_found.clone();
            let truncated = truncated.clone();
            Box::new(move |entry_result| {
                if generation.load(Ordering::Relaxed) != my_gen || truncated.load(Ordering::Relaxed)
                {
                    return WalkState::Quit;
                }
                let Ok(entry) = entry_result else {
                    return WalkState::Continue;
                };
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    return WalkState::Continue;
                }
                let Some(matches) = search_file(entry.path(), &regex) else {
                    return WalkState::Continue;
                };

                let count = matches.len() as u64;
                let _ = tx.send(FileSearchResult {
                    path: entry.path().to_string_lossy().into_owned().into_boxed_str(),
                    matches,
                });
                let total_matches = matches_found.fetch_add(count, Ordering::Relaxed) + count;
                let total_files = files_matched.fetch_add(1, Ordering::Relaxed) + 1;
                if total_files as usize >= MAX_TOTAL_MATCHED_FILES
                    || total_matches as usize >= MAX_TOTAL_MATCHES
                {
                    truncated.store(true, Ordering::Relaxed);
                    return WalkState::Quit;
                }
                WalkState::Continue
            })
        });

        // Every per-thread `Sender` clone above is dropped along with its
        // (now-finished) worker thread by the time `run()` returns; this was
        // the last one left alive, so dropping it lets the aggregator's
        // `recv_timeout` observe `Disconnected`, flush any remainder, and
        // emit `search:done`.
        let _ = aggregator.join();
    });
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn workspace_search(
    app: AppHandle,
    state: State<SearchState>,
    search_id: String,
    root: String,
    options: SearchOptions,
) -> Result<(), SearchError> {
    if options.query.is_empty() {
        // Nothing to search for — still bump the generation so a still-running
        // previous search (e.g. the user just deleted their query text) stops,
        // and tell the frontend this "search" is immediately done/empty.
        state.bump();
        let _ = app.emit(
            "search:done",
            SearchDoneEvent {
                search_id: search_id.into_boxed_str(),
                files_matched: 0,
                matches_found: 0,
                truncated: false,
            },
        );
        return Ok(());
    }

    let regex = build_regex(&options)?;
    let overrides = build_overrides(Path::new(&root), &options.include, &options.exclude)?;

    let my_gen = state.bump();
    let generation = state.handle();

    spawn_search(
        app,
        search_id,
        PathBuf::from(root),
        regex,
        overrides,
        generation,
        my_gen,
    );
    Ok(())
}

/// Cancels whatever search is currently in flight without starting a new
/// one — e.g. the search panel being closed, or the workspace changing.
#[tauri::command]
pub fn stop_workspace_search(state: State<SearchState>) {
    state.bump();
}

#[tauri::command]
pub fn workspace_replace_all(
    paths: Vec<String>,
    options: SearchOptions,
    replacement: String,
) -> Result<Box<[ReplaceFileResult]>, SearchError> {
    let regex = build_regex(&options)?;
    let results = paths
        .into_iter()
        .map(|path| replace_in_file(&path, &regex, &replacement, options.regex))
        .collect::<Vec<_>>();
    Ok(results.into_boxed_slice())
}

/// Re-applies `regex` to the whole file and writes back the replaced
/// content. In regex mode, `replacement` is expanded for `$1`/`$name`
/// capture references (via `Captures::expand`, the same template syntax
/// `@codemirror/search`'s own replace uses); in plain-text mode it's
/// inserted literally, since a literal replacement containing `$` shouldn't
/// be reinterpreted as a capture reference.
fn replace_in_file(
    path: &str,
    regex: &Regex,
    replacement: &str,
    expand_groups: bool,
) -> ReplaceFileResult {
    let path_box: Box<str> = path.into();
    let pb = PathBuf::from(path);

    let bytes = match std::fs::read(&pb) {
        Ok(b) => b,
        Err(e) => {
            return ReplaceFileResult {
                path: path_box,
                replaced: 0,
                error: Some(e.to_string().into_boxed_str()),
            }
        }
    };
    if bytes.len() as u64 > MAX_SEARCHABLE_FILE_BYTES {
        return ReplaceFileResult {
            path: path_box,
            replaced: 0,
            error: Some("File is too large to replace here.".into()),
        };
    }
    let text = match std::str::from_utf8(&bytes) {
        Ok(t) => t,
        Err(_) => {
            return ReplaceFileResult {
                path: path_box,
                replaced: 0,
                error: Some("This doesn't look like a text file.".into()),
            }
        }
    };

    let mut count = 0usize;
    let new_content = if expand_groups {
        regex.replace_all(text, |caps: &regex::Captures| {
            count += 1;
            let mut dst = String::new();
            caps.expand(replacement, &mut dst);
            dst
        })
    } else {
        regex.replace_all(text, |_: &regex::Captures| {
            count += 1;
            replacement.to_string()
        })
    };

    if count > 0 {
        if let Err(e) = std::fs::write(&pb, new_content.as_bytes()) {
            return ReplaceFileResult {
                path: path_box,
                replaced: 0,
                error: Some(e.to_string().into_boxed_str()),
            };
        }
    }
    ReplaceFileResult {
        path: path_box,
        replaced: count,
        error: None,
    }
}
