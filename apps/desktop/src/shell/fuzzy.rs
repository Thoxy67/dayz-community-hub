//! Fuzzy-ish matching for the command palette (= `lib/shortcuts/fuzzy.ts`): a word found
//! whole wins, at the start of a word more; else every word of the query somewhere; else
//! the letters in order where each run starts a word ("dch" finds "DayZ Community Hub"),
//! closer together better. Accents and case do not count. Ranges are in characters
//! (not bytes), so the palette can underline them.

/// A match: its score and the matched character ranges (`start..end`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Match {
    pub score: f32,
    pub ranges: Vec<(usize, usize)>,
}

/// Lower case, accents gone: "Énoch" finds "enoch". One character per character, so
/// positions in the folded text are positions in the original.
pub fn fold(text: &str) -> Vec<char> {
    text.chars().map(fold_char).collect()
}

/// One character folded: lower case, the common Latin accents dropped.
fn fold_char(c: char) -> char {
    let lower = c.to_lowercase().next().unwrap_or(c);
    match lower {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        'ё' => 'е',
        'й' => 'и',
        other => other,
    }
}

fn word_start(t: &[char], i: usize) -> bool {
    i == 0
        || t.get(i - 1).is_some_and(|c| {
            c.is_whitespace() || matches!(c, '-' | '_' | '.' | ':' | '/' | '[' | '(' | '|' | '·')
        })
}

fn find(t: &[char], q: &[char], from: usize) -> Option<usize> {
    if q.is_empty() {
        return Some(from.min(t.len()));
    }
    if q.len() > t.len() {
        return None;
    }
    (from..=t.len() - q.len()).find(|&i| t.get(i..i + q.len()) == Some(q))
}

fn find_char(t: &[char], c: char, from: usize) -> Option<usize> {
    t.iter()
        .enumerate()
        .skip(from)
        .find(|(_, x)| **x == c)
        .map(|(i, _)| i)
}

/// How well `query` (already folded, see [`fold`]) matches `text`; `None` if not at all.
pub fn matches(query: &[char], text: &str) -> Option<Match> {
    if query.is_empty() {
        return Some(Match::default());
    }
    let t = fold(text);
    let len = t.len().min(100) as f32;
    if let Some(at) = find(&t, query, 0) {
        // Whole: 1000, at a word's start +200, at the very start +100, shorter texts first.
        let mut score = 1000.0 - len;
        if word_start(&t, at) {
            score += 200.0;
        }
        if at == 0 {
            score += 100.0;
        }
        return Some(Match {
            score,
            ranges: vec![(at, at + query.len())],
        });
    }
    // Each word of the query found somewhere ("deer pvp" in "PvP Deer Isle").
    let parts: Vec<&[char]> = query
        .split(|c| c.is_whitespace())
        .filter(|w| !w.is_empty())
        .collect();
    if parts.len() > 1 {
        let ranges: Option<Vec<(usize, usize)>> = parts
            .iter()
            .map(|w| find(&t, w, 0).map(|i| (i, i + w.len())))
            .collect();
        if let Some(r) = ranges {
            return Some(Match {
                score: 600.0 - len,
                ranges: merge(r),
            });
        }
    }
    // The letters in order, each new run starting a word: initials ("dch" finds "DayZ
    // Community Hub"), never letters picked out of the middle of words.
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut score = 300.0f32;
    let mut from = 0usize;
    let mut prev: Option<usize> = None;
    for &ch in query.iter().filter(|c| !c.is_whitespace()) {
        let next_to_prev = prev.map(|p| p + 1);
        let mut i = find_char(&t, ch, from);
        if i != next_to_prev {
            while let Some(j) = i {
                if word_start(&t, j) {
                    break;
                }
                i = find_char(&t, ch, j + 1);
            }
        }
        let i = i?;
        if Some(i) == next_to_prev {
            if let Some(last) = ranges.last_mut() {
                last.1 = i + 1;
            }
            score += 5.0;
        } else {
            ranges.push((i, i + 1));
            score -= (i.saturating_sub(from)).min(20) as f32;
        }
        prev = Some(i);
        from = i + 1;
    }
    Some(Match {
        score: score - len / 2.0,
        ranges,
    })
}

fn merge(mut r: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    r.sort_unstable();
    let mut out: Vec<(usize, usize)> = Vec::with_capacity(r.len());
    for x in r {
        match out.last_mut() {
            Some(last) if x.0 <= last.1 => last.1 = last.1.max(x.1),
            _ => out.push(x),
        }
    }
    out
}

/// `text` cut into pieces (byte ranges), each marked whether it matched: for drawing.
pub fn pieces(text: &str, ranges: &[(usize, usize)]) -> Vec<(std::ops::Range<usize>, bool)> {
    // Character index → byte offset.
    let bytes: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(text.len()))
        .collect();
    let at = |c: usize| bytes.get(c).copied().unwrap_or(text.len());
    let mut out = Vec::new();
    let mut pos = 0usize;
    for &(a, b) in ranges {
        let (a, b) = (at(a), at(b));
        if a < pos || b < a {
            continue;
        }
        if a > pos {
            out.push((pos..a, false));
        }
        if b > a {
            out.push((a..b, true));
        }
        pos = b;
    }
    if pos < text.len() {
        out.push((pos..text.len(), false));
    }
    out
}

/// "1.2.3.4:2302" or "host.example:2302": an address to connect to.
pub fn address_of(text: &str) -> Option<(String, u16)> {
    let text = text.trim();
    let (host, port) = text.rsplit_once(':')?;
    if !(2..=5).contains(&port.len()) || !port.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let port: u32 = port.parse().ok()?;
    let port = u16::try_from(port).ok().filter(|p| *p > 0)?;
    let labels: Vec<&str> = host.split('.').collect();
    let ipv4 = labels.len() == 4
        && labels
            .iter()
            .all(|l| (1..=3).contains(&l.len()) && l.bytes().all(|b| b.is_ascii_digit()));
    let name = labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    (ipv4 || name).then(|| (host.to_string(), port))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn m(q: &str, t: &str) -> Option<Match> {
        matches(&fold(q), t)
    }

    #[test]
    fn whole_words_win_and_accents_do_not_count() {
        let a = m("enoch", "Énoch server").unwrap();
        assert_eq!(a.ranges, vec![(0, 5)]);
        assert!(a.score > 1200.0);
        let b = m("och", "Énoch").unwrap();
        assert!(b.score < a.score);
        assert!(m("zzz", "Énoch").is_none());
    }

    #[test]
    fn words_anywhere_and_initials() {
        let a = m("deer pvp", "PvP Deer Isle").unwrap();
        assert_eq!(a.ranges, vec![(0, 3), (4, 8)]);
        let b = m("dch", "DayZ Community Hub").unwrap();
        assert_eq!(b.ranges, vec![(0, 1), (5, 6), (15, 16)]);
        // Letters out of the middle of words do not count.
        assert!(m("ayz", "DayZ").is_some(), "a whole substring");
        assert!(m("dz", "add zebra").is_none());
    }

    #[test]
    fn pieces_follow_characters() {
        let p = pieces("Énoch", &[(1, 3)]);
        assert_eq!(p, vec![(0..2, false), (2..4, true), (4..6, false)]);
    }

    #[test]
    fn addresses() {
        assert_eq!(address_of(" 1.2.3.4:2302 "), Some(("1.2.3.4".into(), 2302)));
        assert_eq!(
            address_of("play.example.com:2402"),
            Some(("play.example.com".into(), 2402))
        );
        assert_eq!(address_of("1.2.3.4:99999"), None);
        assert_eq!(address_of("localhost:2302"), None);
        assert_eq!(address_of("enoch"), None);
    }
}
