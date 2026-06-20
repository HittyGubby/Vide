use pest::error::Error as PestError;
use pest::iterators::Pair;
use pest::Parser;

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

fn get_field_value(song: &Song, field: &str) -> String {
    match field.to_lowercase().as_str() {
        "title" => song.title.clone(),
        "artist" => song.artist.clone(),
        "album" => song.album.clone(),
        "source" => song
            .sources
            .iter()
            .map(|s| s.platform.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        "path" => song
            .paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        "added_time" => song.metadata.added_time.clone().unwrap_or_default(),
        "duration" => song.metadata.duration.map(|d| d.to_string()).unwrap_or_default(),
        "import_order" => song.metadata.import_order.map(|o| o.to_string()).unwrap_or_default(),
        "import_source" => song.metadata.import_source.clone().unwrap_or_default(),
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
                    t.namespace.eq_ignore_ascii_case(namespace)
                        && t.value.eq_ignore_ascii_case(value)
                }),
                None => false,
            }
        }
        QueryAst::TextFilter { field, value } => {
            let haystack = get_field_value(song, field).to_lowercase();
            let needle = value.to_lowercase();
            haystack.contains(&needle)
        }
    }
}
