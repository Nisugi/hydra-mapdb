//! A scripted edge's *shape*: its Ruby with the parameters normalised away.
//!
//! `plan/21` §2b. Upstream stores ~7,900 scripted edges, but most differ only in
//! a table name, a room number or a regex. Normalising those leaves a few
//! hundred shapes, and **a shape is the unit of porting**: one recogniser arm
//! and one piece of Rust crosses every edge that shares it.
//!
//! This is a lexical normaliser, not a Ruby parser. Its job is to cluster, and
//! a wrong cluster costs only a less tidy report -- an edge is never *crossed*
//! on the strength of its shape alone. The recogniser that does cross edges
//! (`plan/21` §5 step 7) reads the source itself.
//!
//! The rules:
//!
//! | Source | Becomes |
//! |---|---|
//! | `'…'` or `"…"`, escapes honoured | `S` |
//! | `/…/flags` where a regex can start | `R` |
//! | a number not inside an identifier | `N` |
//! | `[N, S, nil, …]` | `[..]` |
//! | any run of whitespace | one space |

use cena_map::ShapeId;

/// Normalise a script to its shape.
#[must_use]
pub fn normalise(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            i = skip_delimited(&chars, i, c);
            out.push('S');
        } else if c == '/' && regex_can_start(&out) && closes(&chars, i) {
            i = skip_delimited(&chars, i, '/');
            while i < chars.len() && chars[i].is_ascii_lowercase() {
                i += 1;
            }
            out.push('R');
        } else if c.is_ascii_digit() && !ends_in_identifier(&out) {
            i = skip_number(&chars, i);
            out.push('N');
        } else if c.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    collapse_literal_arrays(out.trim())
}

/// The stable id of a script's shape: FNV-1a 64 over [`normalise`]'s output.
///
/// Hand-rolled because it must never change: these ids are written into the
/// map files and quoted in the porting worklist, and `std`'s hasher is
/// explicitly not stable across releases.
#[must_use]
pub fn shape_id(source: &str) -> ShapeId {
    ShapeId(format!("{:016x}", fnv1a(normalise(source).as_bytes())))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Index just past the closing `delimiter` of the literal opening at `start`.
/// An unterminated literal runs to the end.
fn skip_delimited(chars: &[char], start: usize, delimiter: char) -> usize {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            c if c == delimiter => return i + 1,
            _ => i += 1,
        }
    }
    chars.len()
}

/// Whether the `/` at `start` has a closing `/` before the statement ends --
/// which separates `x =~ /re/` from a stray division at the end of a line.
fn closes(chars: &[char], start: usize) -> bool {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '/' => return i > start + 1,
            '\n' => return false,
            _ => i += 1,
        }
    }
    false
}

/// A `/` opens a regex unless it follows a value, where it is division. After
/// an identifier, a number, `)` or `]` it divides; after an operator, a comma,
/// an opening bracket or nothing at all, it opens a regex. `S`, `R` and `N`
/// count as values because they are what this function has already written.
fn regex_can_start(out: &str) -> bool {
    match out.trim_end().chars().last() {
        None => true,
        Some(c) => !(c.is_alphanumeric() || c == '_' || c == ')' || c == ']'),
    }
}

fn ends_in_identifier(out: &str) -> bool {
    out.chars()
        .last()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Index just past the number at `start`: digits, then at most one `.digits`.
/// `3.times` keeps its `.times`, because the dot is not followed by a digit.
fn skip_number(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
    }
    i
}

/// Replace every `[…]` holding only literals with `[..]`, so a maze's sixteen
/// room ids and another's nine are one shape.
fn collapse_literal_arrays(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find(']') {
            Some(close) if is_literal_list(&after[..close]) => {
                out.push_str("[..]");
                rest = &after[close + 1..];
            }
            _ => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A non-empty, comma-separated list of `N`, `S` or `nil`.
fn is_literal_list(inner: &str) -> bool {
    let mut items = inner
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .peekable();
    items.peek().is_some() && items.all(|item| matches!(item, "N" | "S" | "nil"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_normalise_away() {
        assert_eq!(
            normalise(";e move 'climb rope'; waitrt?"),
            ";e move S; waitrt?"
        );
        assert_eq!(
            normalise(";e move \"go door\";   waitrt?"),
            normalise(";e move 'climb rope'; waitrt?"),
        );
    }

    #[test]
    fn a_regex_is_one_token_and_division_is_not() {
        assert_eq!(
            normalise(";e dothistimeout 'push wall', 3, /you push|you can't push/i; waitrt?"),
            ";e dothistimeout S, N, R; waitrt?",
        );
        assert_eq!(
            normalise(";e Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max"),
            ";e Skills.climbing >= [XMLData.encumbrance_value/N,N].max",
        );
    }

    #[test]
    fn a_quote_inside_a_regex_does_not_open_a_string() {
        assert_eq!(
            normalise(";e x =~ /you can't/; move 'n'"),
            ";e x =~ R; move S"
        );
    }

    #[test]
    fn digits_inside_an_identifier_survive() {
        assert_eq!(
            normalise(";e $go2_restart = true"),
            ";e $go2_restart = true"
        );
        assert_eq!(
            normalise(";e 3.times { move 'n' }"),
            ";e N.times { move S }"
        );
    }

    #[test]
    fn literal_arrays_collapse_whatever_their_length() {
        let sixteen = ";e rooms = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]";
        let nine = ";e rooms = [1, 2, nil, 4, 5, 6, 7, 8, 9]";
        assert_eq!(normalise(sixteen), ";e rooms = [..]");
        assert_eq!(normalise(sixteen), normalise(nine));
        // An index collapses too. Harmless: `Room[7]` and `Room[23282]` are the
        // same shape either way, which is the point.
        assert_eq!(
            normalise(";e Room[7].wayto['30714'].call"),
            ";e Room[..].wayto[..].call"
        );
    }

    #[test]
    fn the_shape_id_is_pinned() {
        // FNV-1a 64 of ";e true". If this changes, every id in every written
        // map file and every line of the porting worklist changes with it.
        assert_eq!(shape_id(";e true").0, format!("{:016x}", fnv1a(b";e true")));
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(shape_id(";e move 'a'"), shape_id(";e move \"bcd\""));
    }
}
