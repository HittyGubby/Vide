use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Bump when the analysis algorithm/thresholds change so cached results from an
/// older version are recomputed.
const ANALYSIS_VERSION: u32 = 1;
const SILENCE_NOISE: &str = "-50dB";
const SILENCE_MIN_SECS: f64 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SongAnalysis {
    /// First non-silent moment (seconds, absolute in the file).
    pub start_secs: f64,
    /// Last non-silent moment (seconds, absolute in the file).
    pub end_secs: f64,
    /// EBU R128 integrated loudness, if measurable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lufs: Option<f64>,
}

#[derive(Serialize, Deserialize)]
struct CachedAnalysis {
    version: u32,
    size: u64,
    mtime: u64,
    start_secs: f64,
    end_secs: f64,
    lufs: Option<f64>,
}

fn cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cache").join("analysis")
}

fn cache_file(data_dir: &Path, song_id: &str) -> PathBuf {
    // Song ids are hashes, but imported JSON could contain anything; never let
    // an id escape the cache directory.
    let safe: String = song_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    cache_dir(data_dir).join(format!("{safe}.json"))
}

fn identity(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some((meta.len(), mtime))
}

/// Loudness target → constant gain, clamped so a quiet file cannot blow up the
/// output and a loud one is not attenuated into nothing.
pub fn gain_db(lufs: Option<f64>, target: f64) -> f64 {
    lufs.map(|value| (target - value).clamp(-12.0, 6.0))
        .unwrap_or(0.0)
}

/// Analyze a media file, reusing the cached result when the file is unchanged.
///
/// Returns `None` when ffmpeg is unavailable or the file cannot be decoded; the
/// player then simply plays without effects.
pub async fn get_or_analyze(
    data_dir: &Path,
    song_id: &str,
    path: &Path,
) -> Option<SongAnalysis> {
    let (size, mtime) = identity(path)?;
    let file = cache_file(data_dir, song_id);

    if let Ok(raw) = tokio::fs::read_to_string(&file).await {
        if let Ok(cached) = serde_json::from_str::<CachedAnalysis>(&raw) {
            if cached.version == ANALYSIS_VERSION && cached.size == size && cached.mtime == mtime {
                return Some(SongAnalysis {
                    start_secs: cached.start_secs,
                    end_secs: cached.end_secs,
                    lufs: cached.lufs,
                });
            }
        }
    }

    let analysis = analyze(path).await?;
    let cached = CachedAnalysis {
        version: ANALYSIS_VERSION,
        size,
        mtime,
        start_secs: analysis.start_secs,
        end_secs: analysis.end_secs,
        lufs: analysis.lufs,
    };
    if let Ok(json) = serde_json::to_string(&cached) {
        if tokio::fs::create_dir_all(cache_dir(data_dir)).await.is_ok() {
            let tmp = file.with_extension("tmp");
            if tokio::fs::write(&tmp, json).await.is_ok() {
                let _ = tokio::fs::rename(&tmp, &file).await;
            }
        }
    }
    Some(analysis)
}

/// One ffmpeg pass: detect silence at both ends and measure integrated loudness.
async fn analyze(path: &Path) -> Option<SongAnalysis> {
    let filter = format!(
        "silencedetect=noise={SILENCE_NOISE}:d={SILENCE_MIN_SECS},ebur128"
    );
    let output = tokio::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-nostdin", "-i"])
        .arg(path)
        .args(["-af", &filter, "-f", "null", "-"])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let duration = parse_duration(&stderr)?;
    let (start_secs, end_secs) = parse_silence(&stderr, duration);
    let lufs = parse_lufs(&stderr);

    if end_secs - start_secs < 1.0 {
        return Some(SongAnalysis { start_secs: 0.0, end_secs: duration, lufs });
    }
    Some(SongAnalysis { start_secs, end_secs, lufs })
}

fn parse_duration(stderr: &str) -> Option<f64> {
    for line in stderr.lines() {
        let Some(index) = line.find("Duration: ") else {
            continue;
        };
        let rest = &line[index + "Duration: ".len()..];
        let value = rest.split(',').next()?.trim();
        let mut parts = value.split(':');
        let hours: f64 = parts.next()?.parse().ok()?;
        let minutes: f64 = parts.next()?.parse().ok()?;
        let seconds: f64 = parts.next()?.parse().ok()?;
        return Some(hours * 3600.0 + minutes * 60.0 + seconds);
    }
    None
}

enum SilenceEvent {
    Start(f64),
    End(f64),
}

fn parse_silence(stderr: &str, duration: f64) -> (f64, f64) {
    let mut events = Vec::new();
    for line in stderr.lines() {
        if let Some(index) = line.find("silence_start: ") {
            if let Ok(value) = line[index + "silence_start: ".len()..]
                .trim()
                .parse::<f64>()
            {
                events.push(SilenceEvent::Start(value));
            }
        } else if let Some(index) = line.find("silence_end: ") {
            let rest = &line[index + "silence_end: ".len()..];
            if let Some(value) = rest.split('|').next() {
                if let Ok(value) = value.trim().parse::<f64>() {
                    events.push(SilenceEvent::End(value));
                }
            }
        }
    }

    let mut start = 0.0;
    let mut end = duration;

    // Leading silence: the first event starts at (almost) zero and is closed.
    if matches!(events.first(), Some(SilenceEvent::Start(value)) if *value <= 0.1) {
        if let Some(SilenceEvent::End(value)) = events.get(1) {
            if *value < duration - 0.2 {
                start = *value;
            }
        }
    }

    // Trailing silence: the last event is an unclosed start, or the last
    // silence ends at the end of the file.
    match events.last() {
        Some(SilenceEvent::Start(value)) => {
            if *value > start + 0.5 {
                end = *value;
            }
        }
        Some(SilenceEvent::End(value)) if *value >= duration - 0.1 => {
            if let Some(SilenceEvent::Start(begin)) = events.get(events.len().saturating_sub(2)) {
                if *begin > start + 0.5 {
                    end = *begin;
                }
            }
        }
        _ => {}
    }

    (start, end)
}

fn parse_lufs(stderr: &str) -> Option<f64> {
    let mut result = None;
    for line in stderr.lines() {
        if !line.contains("LUFS") {
            continue;
        }
        let Some(index) = line.find("I:") else {
            continue;
        };
        let rest = line[index + 2..].trim();
        if let Some(token) = rest.split_whitespace().next() {
            if let Ok(value) = token.parse::<f64>() {
                if value > -70.0 && value.is_finite() {
                    result = Some(value);
                }
            }
        }
    }
    result
}
