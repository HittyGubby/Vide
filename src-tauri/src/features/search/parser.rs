use pest::error::Error as PestError;
use pest::iterators::Pair;
use pest::Parser;
use unicode_normalization::UnicodeNormalization;

use crate::models::song::Song;
use crate::models::tag::Tag;
use crate::models::SongId;

// ── Grammar ───────────────────────────────────────────────────────

#[derive(pest_derive::Parser)]
#[grammar = "src/features/search/grammar.pest"]
pub struct QueryParser;

// ── AST ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum QueryAst {
    Or(Box<QueryAst>, Box<QueryAst>),
    And(Box<QueryAst>, Box<QueryAst>),
    Not(Box<QueryAst>),
    TagFilter { namespace: String, value: String },
    TextFilter { field: String, value: String },
    /// Comparison against a built-in field: duration > 300, added_time >=
    /// "2026-01-01", duration < 200, or string equality.
    Compare { field: String, op: String, value: String },
}

// ── Parsing ───────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ParseError {
    Pest(PestError<Rule>),
    Empty,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Pest(e) => write!(f, "parse error: {e}"),
            ParseError::Empty => write!(f, "empty query"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<PestError<Rule>> for ParseError {
    fn from(e: PestError<Rule>) -> Self {
        ParseError::Pest(e)
    }
}

/// Parse a query string into an AST.
pub fn parse_query(input: &str) -> Result<QueryAst, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut pairs = QueryParser::parse(Rule::query, input)?;
    let pair = pairs.next().unwrap();
    build_ast(pair)
}

fn build_ast(pair: Pair<Rule>) -> Result<QueryAst, ParseError> {
    match pair.as_rule() {
        Rule::query | Rule::expr => {
            let inner = pair.into_inner().next().unwrap();
            build_ast(inner)
        }
        Rule::or_expr => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();
            let mut left = build_ast(first)?;

            while let Some(_op) = inner.next() {
                let right_pair = inner.next().unwrap();
                let right = build_ast(right_pair)?;
                left = QueryAst::Or(Box::new(left), Box::new(right));
            }
            Ok(left)
        }
        Rule::and_expr => {
            let mut inner = pair.into_inner();
            let first = inner.next().unwrap();
            let mut left = build_ast(first)?;

            while let Some(_op) = inner.next() {
                let right_pair = inner.next().unwrap();
                let right = build_ast(right_pair)?;
                left = QueryAst::And(Box::new(left), Box::new(right));
            }
            Ok(left)
        }
        Rule::atom => {
            let inner = pair.into_inner().next().unwrap();
            build_ast(inner)
        }
        Rule::paren_expr => {
            let inner = pair.into_inner().next().unwrap();
            build_ast(inner)
        }
        Rule::not_expr => {
            let mut inner = pair.into_inner();
            inner.next(); // skip NOT/!
            let child = inner.next().unwrap();
            let child_ast = build_ast(child)?;
            Ok(QueryAst::Not(Box::new(child_ast)))
        }
        Rule::filter => {
            let inner = pair.into_inner().next().unwrap();
            build_ast(inner)
        }
        Rule::tag_filter => {
            let mut inner = pair.into_inner();
            let namespace = inner.next().unwrap().as_str().to_string();
            let value = unescape(inner.next().unwrap());
            Ok(QueryAst::TagFilter { namespace, value })
        }
        Rule::text_filter => {
            let mut inner = pair.into_inner();
            let field = inner.next().unwrap().as_str().to_string();
            let value = unescape(inner.next().unwrap());
            Ok(QueryAst::TextFilter { field, value })
        }
        Rule::cmp_filter => {
            let mut inner = pair.into_inner();
            let field = inner.next().unwrap().as_str().to_string();
            let op = inner.next().unwrap().as_str().to_string();
            let value = unescape(inner.next().unwrap());
            Ok(QueryAst::Compare { field, op, value })
        }
        other => {
            panic!("unexpected rule: {other:?}")
        }
    }
}

fn unescape(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    match pair.as_rule() {
        Rule::quoted_string => {
            let inner = &s[1..s.len() - 1];
            let mut result = String::with_capacity(inner.len());
            let mut chars = inner.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    if let Some(next) = chars.next() {
                        result.push(next);
                    }
                } else {
                    result.push(c);
                }
            }
            result
        }
        _ => s.to_string(),
    }
}

// ── Evaluation ────────────────────────────────────────────────────

type TagIndex = std::collections::HashMap<String, Vec<Tag>>;

/// Build a tag index from a list of (song_id, tag) pairs.
pub fn build_tag_index(tags: &[(SongId, Tag)]) -> TagIndex {
    let mut idx: TagIndex = std::collections::HashMap::new();
    for (song_id, tag) in tags {
        idx.entry(song_id.0.clone()).or_default().push(tag.clone());
    }
    idx
}

/// Evaluate a QueryAst against a list of songs with their tags.
/// Returns the matching songs.
pub fn evaluate(ast: &QueryAst, songs: &[Song], tag_index: &TagIndex) -> Vec<Song> {
    songs
        .iter()
        .filter(|song| matches(song, tag_index, ast))
        .cloned()
        .collect()
}

/// Lowercase and strip combining marks, so "Björk" matches "bjork".
pub fn fold(input: &str) -> String {
    input
        .nfd()
        .filter(|ch| !unicode_normalization::char::is_combining_mark(*ch))
        .collect::<String>()
        .to_lowercase()
}

fn get_field_value(song: &Song, field: &str) -> String {
    match field.to_lowercase().as_str() {
        "id" => song.id.0.clone(),
        // Include the display override and the raw tag value so both remain
        // searchable after the user renames a song.
        "title" => match &song.metadata.display_title {
            Some(display) => format!("{} {}", song.title, display),
            None => song.title.clone(),
        },
        "artist" => match &song.metadata.artist_override {
            Some(display) => format!("{} {}", song.artist, display),
            None => song.artist.clone(),
        },
        "album" => match &song.metadata.album_override {
            Some(display) => format!("{} {}", song.album, display),
            None => song.album.clone(),
        },
        "tags" => song
            .tags
            .iter()
            .map(|tag| format!("{}:{}", tag.namespace, tag.value))
            .collect::<Vec<_>>()
            .join(" "),
        "source" => song
            .sources
            .iter()
            .map(|s| s.platform.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        "source_id" => song
            .sources
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        "path" => song
            .paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        "file_name" => song.metadata.file_name.clone().unwrap_or_default(),
        "notes" => song.metadata.notes.clone().unwrap_or_default(),
        "lyrics" => song.metadata.lyrics.clone().unwrap_or_default(),
        "added_time" => song.metadata.added_time.clone().unwrap_or_default(),
        "duration" => song.metadata.duration.map(|d| d.to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}

fn matches(song: &Song, tag_index: &TagIndex, ast: &QueryAst) -> bool {
    match ast {
        QueryAst::Or(left, right) => {
            matches(song, tag_index, left) || matches(song, tag_index, right)
        }
        QueryAst::And(left, right) => {
            matches(song, tag_index, left) && matches(song, tag_index, right)
        }
        QueryAst::Not(inner) => !matches(song, tag_index, inner),
        QueryAst::TagFilter { namespace, value } => {
            let song_tags = tag_index.get(&song.id.0);
            match song_tags {
                Some(tags) => tags.iter().any(|t| {
                    fold(&t.namespace) == fold(namespace) && fold(&t.value) == fold(value)
                }),
                None => false,
            }
        }
        QueryAst::TextFilter { field, value } => {
            // CONTAINS is deliberately loose: split the value into words and
            // require every word (AND), ignoring case and diacritics.
            // "song_metadata" is a fixed macro over the three display fields;
            // each word may appear in any one of them.
            let needles: Vec<String> = value
                .split_whitespace()
                .map(fold)
                .filter(|needle| !needle.is_empty())
                .collect();
            if field.eq_ignore_ascii_case("song_metadata") {
                needles.iter().all(|needle| {
                    ["title", "artist", "album"]
                        .iter()
                        .any(|name| fold(&get_field_value(song, name)).contains(needle))
                })
            } else {
                let haystack = fold(&get_field_value(song, field));
                needles.iter().all(|needle| haystack.contains(needle))
            }
        }
        QueryAst::Compare { field, op, value } => {
            let left = get_field_value(song, field);
            let numeric = matches!(field.to_lowercase().as_str(), "duration");
            let ordering = if numeric {
                match (
                    left.trim().parse::<f64>(),
                    value.trim().parse::<f64>(),
                ) {
                    (Ok(a), Ok(b)) => a.partial_cmp(&b),
                    _ => None,
                }
            } else {
                Some(fold(&left).cmp(&fold(value)))
            };
            match ordering {
                Some(ord) => match op.as_str() {
                    "=" => ord == std::cmp::Ordering::Equal,
                    "!=" => ord != std::cmp::Ordering::Equal,
                    ">" => ord == std::cmp::Ordering::Greater,
                    ">=" => ord != std::cmp::Ordering::Less,
                    "<" => ord == std::cmp::Ordering::Less,
                    "<=" => ord != std::cmp::Ordering::Greater,
                    _ => false,
                },
                // Missing duration never satisfies a comparison.
                None => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::song::{Song, SongMetadata};

    fn song(id: &str, title: &str, artist: &str, album: &str) -> Song {
        Song {
            id: SongId(id.to_string()),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            tags: Vec::new(),
            sources: Vec::new(),
            paths: Vec::new(),
            metadata: SongMetadata::default(),
        }
    }

    fn titles(query: &str, songs: &[Song]) -> Vec<String> {
        let ast = parse_query(query).expect("query should parse");
        let tags = TagIndex::new();
        evaluate(&ast, songs, &tags)
            .into_iter()
            .map(|song| song.title)
            .collect()
    }

    #[test]
    fn contains_requires_every_word() {
        let songs = vec![
            song("1", "alpha beta", "x", "y"),
            song("2", "alpha", "x", "y"),
        ];
        assert_eq!(titles("title CONTAINS \"alpha beta\"", &songs), vec!["alpha beta"]);
    }

    #[test]
    fn contains_ignores_case_and_diacritics() {
        let songs = vec![song("1", "Héllo Wörld", "Björk", "Álbum")];
        assert_eq!(titles("title CONTAINS \"HELLO\"", &songs), vec!["Héllo Wörld"]);
        // Operators are case-insensitive.
        assert_eq!(titles("title contains \"hello\"", &songs), vec!["Héllo Wörld"]);
        assert_eq!(titles("artist CONTAINS \"bjork\"", &songs), vec!["Héllo Wörld"]);
    }

    #[test]
    fn song_metadata_searches_title_artist_and_album() {
        let songs = vec![song("1", "Héllo", "Björk", "Álbum")];
        // Each word may come from a different field.
        assert_eq!(
            titles("song_metadata CONTAINS \"hello bjork album\"", &songs),
            vec!["Héllo"]
        );
        // But every word must match somewhere.
        assert!(titles("song_metadata CONTAINS \"hello missing\"", &songs).is_empty());
    }

    #[test]
    fn display_overrides_are_searchable_alongside_raw_values() {
        let mut item = song("1", "Raw Title", "Raw Artist", "Raw Album");
        item.metadata.display_title = Some("Nice Title".to_string());
        item.metadata.artist_override = Some("Nice Artist".to_string());
        let songs = vec![item];

        assert_eq!(titles("title CONTAINS \"nice\"", &songs), vec!["Raw Title"]);
        assert_eq!(titles("title CONTAINS \"raw\"", &songs), vec!["Raw Title"]);
        assert_eq!(
            titles("song_metadata CONTAINS \"nice artist\"", &songs),
            vec!["Raw Title"]
        );
    }

    #[test]
    fn comparison_filters_work_for_numbers_and_dates() {
        let mut short = song("1", "Short", "a", "b");
        short.metadata.duration = Some(120.0);
        short.metadata.added_time = Some("2025-01-01".to_string());
        let mut long = song("2", "Long", "a", "b");
        long.metadata.duration = Some(420.0);
        long.metadata.added_time = Some("2026-06-01".to_string());
        let songs = vec![short, long];

        assert_eq!(
            titles("duration > 300", &songs),
            vec!["Long"]
        );
        assert_eq!(
            titles("duration <= 120", &songs),
            vec!["Short"]
        );
        assert_eq!(
            titles("added_time >= \"2026-01-01\"", &songs),
            vec!["Long"]
        );
        assert_eq!(titles("title = \"short\"", &songs), vec!["Short"]);
        assert_eq!(titles("title != \"short\"", &songs), vec!["Long"]);
    }

    #[test]
    fn empty_contains_matches_everything() {
        let songs = vec![song("1", "one", "a", "b"), song("2", "two", "c", "d")];
        assert_eq!(titles("title CONTAINS \"\"", &songs).len(), 2);
    }

    #[test]
    fn tags_can_be_queried_as_joined_text() {
        use crate::models::tag::Tag;

        let mut songs = vec![song("1", "one", "a", "b")];
        songs[0].tags.push(Tag {
            namespace: "genre".to_string(),
            value: "folk".to_string(),
        });
        let ast = parse_query("tags CONTAINS \"genre:folk\"").unwrap();
        assert_eq!(evaluate(&ast, &songs, &TagIndex::new()).len(), 1);
        let missing = parse_query("tags CONTAINS \"genre:pop\"").unwrap();
        assert!(evaluate(&missing, &songs, &TagIndex::new()).is_empty());
    }

    #[test]
    fn tag_filters_are_loose_about_case_and_diacritics() {
        use crate::models::tag::Tag;

        let songs = vec![song("1", "one", "a", "b")];
        let mut tags = TagIndex::new();
        tags.insert(
            "1".to_string(),
            vec![Tag {
                namespace: "country".to_string(),
                value: "Rússia".to_string(),
            }],
        );
        let ast = parse_query("country:russia").unwrap();
        assert_eq!(evaluate(&ast, &songs, &tags).len(), 1);
    }
}
