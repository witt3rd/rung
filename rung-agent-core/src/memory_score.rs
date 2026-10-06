//! Score memory providers on a directory of notes and a questions file.
//!
//! ```text
//! rung-agent --memory-score --notes DIR --questions FILE
//!            [--arm SETTING]... [--no-baseline] [--timeout SECS] [--misses]
//! ```
//!
//! `DIR` holds either `notes.jsonl` (`{"id","text"}` per line) or one note
//! per `*.md` / `*.txt` file (the id is the file stem). `FILE` is JSONL,
//! `{"q": "...", "expect": ["note-id", ...]}` per line. Each arm (the
//! baseline provider, plus every `--arm`, e.g. `mcp:http://127.0.0.1:9000/mcp`)
//! retains every note into a scope of its own, then recalls each question
//! sequentially. The report is one Markdown page of hit@1, hit@5, MRR,
//! recall latency and cost per arm. The notes never leave the process except
//! to the providers named.
//!
//! A hit is a recalled record that is an expected note: by the id the
//! provider returned when the note was retained, else by the record's text
//! containing the note's text (a provider that rewrites ids still scores).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rung_memory::{
    Body, Cue, Kept, MemoryAuthority, MemoryProvider, Observation, ProviderSettings, Scope, Token,
};

/// One note to retain.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Note {
    pub id: String,
    pub text: String,
}

/// One question and the note ids that answer it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Question {
    pub q: String,
    pub expect: Vec<String>,
}

/// What to score.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub notes: String,
    pub questions: String,
    /// Provider settings besides the baseline.
    pub arms: Vec<String>,
    pub no_baseline: bool,
    pub timeout_secs: Option<u64>,
    /// List the questions an arm missed (they may be private: off by default).
    pub misses: bool,
}

impl Options {
    /// Parse the arguments after `--memory-score`.
    pub fn parse(argv: &[String]) -> Result<Self, String> {
        let mut o = Self::default();
        let mut it = argv.iter();
        while let Some(a) = it.next() {
            let mut val = |what: &str| it.next().cloned().ok_or(format!("{a} needs {what}"));
            match a.as_str() {
                "--notes" => o.notes = val("a directory")?,
                "--questions" => o.questions = val("a file")?,
                "--arm" => o.arms.push(val("a provider setting")?),
                "--no-baseline" => o.no_baseline = true,
                "--misses" => o.misses = true,
                "--timeout" => {
                    o.timeout_secs = Some(
                        val("seconds")?
                            .parse()
                            .map_err(|_| "--timeout needs a number".to_string())?,
                    )
                }
                other => return Err(format!("memory score: unknown option {other}")),
            }
        }
        if o.notes.is_empty() || o.questions.is_empty() {
            return Err("memory score: --notes DIR and --questions FILE are required".into());
        }
        if o.no_baseline && o.arms.is_empty() {
            return Err("memory score: --no-baseline leaves no arm to score".into());
        }
        Ok(o)
    }
}

/// Notes from `dir`: `notes.jsonl`, else one note per `.md` / `.txt` file.
pub fn load_notes(dir: &Path) -> Result<Vec<Note>, String> {
    let jsonl = dir.join("notes.jsonl");
    if jsonl.is_file() {
        return jsonl_lines(&jsonl);
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file() && matches!(p.extension().and_then(|e| e.to_str()), Some("md" | "txt"))
        })
        .collect();
    files.sort();
    let mut notes = Vec::new();
    for p in files {
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if text.trim().is_empty() {
            continue;
        }
        let id = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        notes.push(Note { id, text });
    }
    Ok(notes)
}

fn jsonl_lines<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>, String> {
    let body = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    body.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            serde_json::from_str(l).map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))
        })
        .collect()
}

/// Questions from a JSONL file.
pub fn load_questions(path: &Path) -> Result<Vec<Question>, String> {
    jsonl_lines(path)
}

fn norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// One arm's result.
#[derive(Debug, Clone, Default)]
pub struct Arm {
    pub setting: String,
    pub notes: usize,
    pub stored: usize,
    pub questions: usize,
    pub hit1: f64,
    pub hit5: f64,
    pub mrr: f64,
    pub latencies_ms: Vec<u64>,
    pub recall_cost_usd: f64,
    pub retain_cost_usd: f64,
    /// Recalls the provider failed (counted as a miss).
    pub unavailable: usize,
    pub missed: Vec<String>,
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let i = ((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len());
    sorted[i - 1]
}

/// Score one provider.
pub fn score_arm(
    setting: &str,
    provider: Arc<dyn MemoryProvider>,
    notes: &[Note],
    questions: &[Question],
) -> Arm {
    let scope = Scope::new(format!(
        "rung-memory-score:{}",
        uuid::Uuid::new_v4().simple()
    ));
    let mut arm = Arm {
        setting: setting.to_string(),
        notes: notes.len(),
        questions: questions.len(),
        ..Arm::default()
    };
    let mut by_record: HashMap<String, String> = HashMap::new();
    for n in notes {
        let obs = Observation {
            body: Body::Note {
                text: n.text.clone(),
            },
            attrs: BTreeMap::from([("source".into(), "memory-score".into())]),
        };
        if let Ok(c) = provider.retain(&scope, &obs) {
            arm.retain_cost_usd += c.cost_usd;
            if let Kept::Stored(id) = c.value {
                by_record.insert(id.as_str().to_string(), n.id.clone());
                arm.stored += 1;
            }
        }
    }
    let texts: Vec<(String, &str)> = notes
        .iter()
        .map(|n| (norm(&n.text), n.id.as_str()))
        .collect();
    for q in questions {
        let t = Instant::now();
        let got = provider.recall(&scope, &Cue::new(&q.q));
        arm.latencies_ms
            .push(u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX));
        let hits = match got {
            Ok(c) => {
                arm.recall_cost_usd += c.cost_usd;
                c.value
            }
            Err(m) => {
                arm.recall_cost_usd += m.cost_usd;
                arm.unavailable += 1;
                Vec::new()
            }
        };
        let rank = hits.iter().take(5).position(|h| {
            let text = norm(&h.record.text);
            let owner = by_record.get(h.record.id.as_str()).map(String::as_str);
            q.expect.iter().any(|e| {
                owner == Some(e.as_str())
                    || texts
                        .iter()
                        .any(|(t, id)| *id == e && !t.is_empty() && text.contains(t.as_str()))
            })
        });
        match rank {
            Some(r) => {
                arm.hit1 += f64::from(u8::from(r == 0));
                arm.hit5 += 1.0;
                arm.mrr += 1.0 / (r as f64 + 1.0);
            }
            None => arm.missed.push(q.q.clone()),
        }
    }
    let n = questions.len().max(1) as f64;
    arm.hit1 /= n;
    arm.hit5 /= n;
    arm.mrr /= n;
    arm.latencies_ms.sort_unstable();
    arm
}

/// The one-page Markdown report.
pub fn render(arms: &[Arm], misses: bool) -> String {
    let mut s = String::new();
    let (notes, qs) = arms.first().map_or((0, 0), |a| (a.notes, a.questions));
    s.push_str("## Memory recall report\n\n");
    s.push_str(&format!(
        "{notes} notes, {qs} questions, recall sequential, top 5.\n\n"
    ));
    s.push_str("| arm | stored | hit@1 | hit@5 | MRR | p50 ms | p95 ms | max ms | unavailable | recall $ | retain $ |\n");
    s.push_str("|---|---|---|---|---|---|---|---|---|---|---|\n");
    for a in arms {
        s.push_str(&format!(
            "| {} | {}/{} | {:.3} | {:.3} | {:.3} | {} | {} | {} | {} | {:.4} | {:.4} |\n",
            a.setting,
            a.stored,
            a.notes,
            a.hit1,
            a.hit5,
            a.mrr,
            percentile(&a.latencies_ms, 0.5),
            percentile(&a.latencies_ms, 0.95),
            a.latencies_ms.last().copied().unwrap_or(0),
            a.unavailable,
            a.recall_cost_usd,
            a.retain_cost_usd,
        ));
    }
    s.push_str("\nhit@k: share of questions whose expected note is in the top k recalled; MRR: mean 1/rank of it. Cost is what the provider reported.\n");
    if misses {
        for a in arms {
            if !a.missed.is_empty() {
                s.push_str(&format!("\nMissed by {}:\n", a.setting));
                for q in &a.missed {
                    s.push_str(&format!("- {q}\n"));
                }
            }
        }
    }
    s
}

/// Run the score and return the report.
pub fn run(opts: &Options, token: Option<Token>) -> Result<String, String> {
    let notes = load_notes(Path::new(&opts.notes))?;
    if notes.is_empty() {
        return Err(format!("no notes found in {}", opts.notes));
    }
    let questions = load_questions(Path::new(&opts.questions))?;
    if questions.is_empty() {
        return Err(format!("no questions in {}", opts.questions));
    }
    let mut settings: Vec<String> = Vec::new();
    if !opts.no_baseline {
        settings.push("baseline".into());
    }
    settings.extend(opts.arms.iter().cloned());
    let dir = std::env::temp_dir().join(format!(
        "rung-memory-score-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let registry = crate::memory::registry();
    let mut arms = Vec::new();
    let mut result = Ok(());
    for setting in &settings {
        let built = MemoryAuthority::parse(setting).and_then(|a| match a {
            MemoryAuthority::Provider { name, arg } => registry.build(
                &name,
                &ProviderSettings {
                    dir: dir.join(arms.len().to_string()),
                    arg,
                    timeout: Duration::from_secs(opts.timeout_secs.unwrap_or(30)),
                    retain_timeout: Duration::from_secs(60),
                    token: token.clone(),
                },
            ),
            _ => Err(format!("'{setting}' is a memory setting, not a provider")),
        });
        match built {
            Ok(p) => {
                let _ = std::fs::create_dir_all(dir.join(arms.len().to_string()));
                arms.push(score_arm(setting, p, &notes, &questions));
            }
            Err(e) => {
                result = Err(format!("arm {setting}: {e}"));
                break;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    result?;
    Ok(render(&arms, opts.misses))
}
