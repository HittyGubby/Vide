use crate::models::song::Song;

/// Result of a fuzzy search.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub song: Song,
    pub score: u32,
}

/// Simple fuzzy search that matches songs by substring with tolerance.
///
/// Scans title, artist, and album for the query text.
/// Scoring:
/// - Exact title match: 100 points
/// - Title contains query: 80 + position bonus
/// - Artist contains query: 60
/// - Album contains query: 40
pub fn fuzzy_search(query: &str, songs: &[Song]) -> Vec<FuzzyMatch> {
    let query = query.to_lowercase();
    let mut results: Vec<FuzzyMatch> = Vec::new();

    for song in songs {
        let title = song.title.to_lowercase();
        let artist = song.artist.to_lowercase();
        let album = song.album.to_lowercase();

        let score = compute_score(&query, &title, &artist, &album);
        if score > 0 {
            results.push(FuzzyMatch {
                song: song.clone(),
                score,
            });
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

fn compute_score(query: &str, title: &str, artist: &str, album: &str) -> u32 {
    let mut score = 0u32;

    // Title match bonuses
    if title == query {
        score += 100; // exact title match
    } else if title.starts_with(query) {
        score += 90; // prefix match
    } else if title.contains(query) {
        score += 80; // substring match
    } else if similarity(title, query) > 0.7 {
        score += 50; // fuzzy character overlap
    }

    // Artist match bonuses (lower weight)
    if artist == query {
        score += 70;
    } else if artist.contains(query) {
        score += 60;
    } else if similarity(artist, query) > 0.7 {
        score += 30;
    }

    // Album match bonuses (lowest weight)
    if album == query {
        score += 50;
    } else if album.contains(query) {
        score += 40;
    } else if similarity(album, query) > 0.7 {
        score += 20;
    }

    score
}

/// Simple character overlap similarity (Jaccard-like).
/// Ratio of shared characters to total unique characters.
fn similarity(a: &str, b: &str) -> f64 {
    let set_a: std::collections::HashSet<char> = a.chars().collect();
    let set_b: std::collections::HashSet<char> = b.chars().collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}
