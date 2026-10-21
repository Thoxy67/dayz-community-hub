use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};

use super::format::*;
use super::*;

fn all_words(l: Locale) -> BTreeMap<String, BTreeMap<String, String>> {
    serde_json::from_str(l.source()).unwrap_or_else(|e| panic!("{}.json: {e}", l.code()))
}

#[test]
fn every_language_parses() {
    for l in Locale::ALL {
        let w = parse(l.source()).unwrap_or_else(|e| panic!("{}.json: {e}", l.code()));
        assert!(w.len() >= 20, "{}: only {} dictionaries", l.code(), w.len());
    }
}

/// Missing translations are reported (the source may lack them; English stands in at run
/// time). A word that only a translation has is a stale export and fails.
#[test]
fn every_language_has_every_english_word() {
    let en = all_words(Locale::En);
    let mut orphans = Vec::new();
    for l in Locale::ALL.into_iter().filter(|l| *l != Locale::En) {
        let words = all_words(l);
        let mut missing = Vec::new();
        for (d, ws) in &en {
            for k in ws.keys() {
                if words.get(d).and_then(|w| w.get(k)).is_none() {
                    missing.push(format!("{d}.{k}"));
                }
            }
        }
        if !missing.is_empty() {
            eprintln!(
                "{}: {} words fall back to English: {}",
                l.code(),
                missing.len(),
                missing.join(", ")
            );
        }
        for (d, ws) in &words {
            for k in ws.keys() {
                if en.get(d).and_then(|w| w.get(k)).is_none() {
                    orphans.push(format!("{}: {d}.{k}", l.code()));
                }
            }
        }
    }
    assert!(orphans.is_empty(), "words missing in English: {orphans:?}");
}

#[test]
fn every_placeholder_exists_in_english() {
    let en = all_words(Locale::En);
    let mut bad = Vec::new();
    for l in Locale::ALL {
        for (d, ws) in all_words(l) {
            for (k, text) in ws {
                let Some(source) = en.get(&d).and_then(|w| w.get(&k)) else {
                    continue;
                };
                let known = placeholders(source);
                for p in placeholders(&text) {
                    if !known.contains(&p) {
                        bad.push(format!("{}: {d}.{k} has {{{p}}}", l.code()));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// Every `t("dict", "key")` / `tf("dict", "key", …)` / `t_in(l, "dict", "key")` with
/// literal arguments in the window's sources names an English word: a typo fails here.
#[test]
fn every_literal_key_in_the_sources_exists() {
    let mut files = Vec::new();
    collect_rs(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    let mut bad = Vec::new();
    let mut seen = 0usize;
    // This file writes example calls on purpose.
    for f in files.iter().filter(|f| !f.ends_with("i18n/tests.rs")) {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        for (dict, key) in literal_calls(&src) {
            seen += 1;
            if !has(&dict, &key) {
                bad.push(format!("{}: {dict}.{key}", f.display()));
            }
        }
    }
    assert!(seen > 0, "the scan found no call at all");
    assert!(bad.is_empty(), "unknown words: {bad:?}");
}

fn collect_rs(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `(dict, key)` of each call to `t(`, `tf(` or `t_in(_, ` whose two word arguments are
/// string literals.
fn literal_calls(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let ident_before = i > 0
            && b.get(i - 1)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
        let rest = src.get(i..).unwrap_or("");
        let skip = if ident_before {
            None
        } else if rest.starts_with("t(") {
            Some(2)
        } else if rest.starts_with("tf(") {
            Some(3)
        } else if rest.starts_with("t_in(") {
            // Skip the language argument.
            rest.find(',')
                .filter(|c| !rest.get(..*c).unwrap_or("").contains(')'))
                .map(|c| c + 1)
        } else {
            None
        };
        if let Some(n) = skip
            && let Some((dict, after)) = literal(rest.get(n..).unwrap_or(""))
            && let Some(after) = after.trim_start().strip_prefix(',')
            && let Some((key, _)) = literal(after)
        {
            out.push((dict, key));
        }
        i += src
            .get(i..)
            .and_then(|s| s.chars().next())
            .map_or(1, char::len_utf8);
    }
    out
}

/// A plain `"…"` literal (no escapes) at the start, and what follows it.
fn literal(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start().strip_prefix('"')?;
    let end = s.find('"')?;
    let lit = s.get(..end)?;
    if lit.contains('\\') {
        return None;
    }
    Some((lit.to_owned(), s.get(end + 1..)?))
}

#[test]
fn the_scanner_reads_calls() {
    let src = r#"x(t("common", "close")); tf( "servers","loaded", &[]); t_in(Locale::Fr, "a", "b"); at("no", "no"); t(dict, "k")"#;
    let got = literal_calls(src);
    let want: Vec<(String, String)> = [("common", "close"), ("servers", "loaded"), ("a", "b")]
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn lookup_and_fallbacks() {
    assert_eq!(t_in(Locale::En, "common", "close"), "Close");
    assert_eq!(t_in(Locale::Fr, "common", "close"), "Fermer");
    assert_eq!(t_in(Locale::Ru, "common", "close"), "Закрыть");
    let (dict, key) = (String::from("common"), String::from("noSuchWord"));
    assert_eq!(t_in(Locale::De, &dict, &key), "noSuchWord");
    let (dict, key) = (String::from("noSuchDict"), String::from("close"));
    assert_eq!(t_in(Locale::Es, &dict, &key), "close");
}

#[test]
fn switching_language() {
    // The only test that touches the current language.
    set_locale(Locale::Fr);
    assert_eq!(locale(), Locale::Fr);
    assert_eq!(t("common", "cancel"), "Annuler");
    set_locale(Locale::En);
    assert_eq!(t("common", "cancel"), "Cancel");
}

#[test]
fn inserts() {
    let s = fill(
        "Loaded {count} servers from {where}",
        &[("count", &9499), ("where", &"{count}")],
    );
    assert_eq!(s, "Loaded 9499 servers from {count}");
    assert_eq!(
        fill("{missing} and { not } {", &[]),
        "{missing} and { not } {"
    );
    assert_eq!(fill("é{n}ü", &[("n", &1)]), "é1ü");
    assert_eq!(placeholders("{a} {b_2} { c } {a}"), vec!["a", "b_2", "a"]);
}

#[test]
fn plurals() {
    assert_eq!(plural(1, "a", "b"), "a");
    assert_eq!(plural(0, "a", "b"), "b");
    assert_eq!(plural(3, "a", "b"), "b");
}

#[test]
fn locales() {
    for l in Locale::ALL {
        assert_eq!(Locale::from_code(l.code()), Some(l));
    }
    assert_eq!(Locale::from_code("it"), None);
    assert_eq!(Locale::Ru.label(), "Русский");
    assert_eq!(pick_locale(Some("ru"), Some("fr-FR")), Locale::Ru);
    assert_eq!(pick_locale(Some("xx"), Some("de-DE")), Locale::De);
    assert_eq!(pick_locale(None, Some("fr-CA")), Locale::Fr);
    assert_eq!(pick_locale(None, Some("fr_FR.UTF-8")), Locale::Fr);
    assert_eq!(pick_locale(None, Some("ES")), Locale::Es);
    assert_eq!(pick_locale(None, Some("ja-JP")), Locale::En);
    assert_eq!(pick_locale(None, None), Locale::En);
    assert_eq!(pick_locale(None, Some("")), Locale::En);
}

const NB: &str = "\u{a0}";
const NNB: &str = "\u{202f}";

/// What ICU (`Intl.NumberFormat`) prints, for each language.
#[test]
fn numbers_match_intl() {
    let cases: [(f64, [&str; 5]); 8] = [
        (0.0, ["0", "0", "0", "0", "0"]),
        (999.5, ["999.5", "999,5", "999,5", "999,5", "999,5"]),
        (
            1234.0,
            ["1,234", "1\u{202f}234", "1.234", "1234", "1\u{a0}234"],
        ),
        (
            12345.0,
            ["12,345", "12\u{202f}345", "12.345", "12.345", "12\u{a0}345"],
        ),
        (
            -1234.0,
            ["-1,234", "-1\u{202f}234", "-1.234", "-1234", "-1\u{a0}234"],
        ),
        (
            1234.5678,
            [
                "1,234.568",
                "1\u{202f}234,568",
                "1.234,568",
                "1234,568",
                "1\u{a0}234,568",
            ],
        ),
        (0.5, ["0.5", "0,5", "0,5", "0,5", "0,5"]),
        (
            1_500_000_000.0,
            [
                "1,500,000,000",
                "1\u{202f}500\u{202f}000\u{202f}000",
                "1.500.000.000",
                "1.500.000.000",
                "1\u{a0}500\u{a0}000\u{a0}000",
            ],
        ),
    ];
    for (v, want) in cases {
        for (l, w) in Locale::ALL.into_iter().zip(want) {
            assert_eq!(num_in(l, v), w, "{} {v}", l.code());
        }
    }
    let _ = NNB;
}

#[test]
fn compact_matches_intl() {
    let k = |s: &str| s.replace('~', NB);
    let cases: [(f64, [&str; 5]); 14] = [
        (42.0, ["42", "42", "42", "42", "42"]),
        (999.5, ["999.5", "999,5", "999,5", "999,5", "999,5"]),
        (1000.0, ["1K", "1~k", "1000", "1~mil", "1~тыс."]),
        (1234.0, ["1.2K", "1,2~k", "1234", "1,2~mil", "1,2~тыс."]),
        (9499.0, ["9.5K", "9,5~k", "9499", "9,5~mil", "9,5~тыс."]),
        (9950.0, ["10K", "10~k", "9950", "10~mil", "10~тыс."]),
        (
            18432.0,
            ["18.4K", "18,4~k", "18.432", "18,4~mil", "18,4~тыс."],
        ),
        (99999.0, ["100K", "100~k", "99.999", "100~mil", "100~тыс."]),
        (
            258_600.0,
            ["258.6K", "258,6~k", "258.600", "258,6~mil", "258,6~тыс."],
        ),
        (999_999.0, ["1M", "1~M", "999.999", "1~M", "1~млн"]),
        (
            1_250_000.0,
            ["1.3M", "1,3~M", "1,3~Mio.", "1,3~M", "1,3~млн"],
        ),
        (
            1_500_000_000.0,
            ["1.5B", "1,5~Md", "1,5~Mrd.", "1500~M", "1,5~млрд"],
        ),
        (
            -1234.0,
            ["-1.2K", "-1,2~k", "-1234", "-1,2~mil", "-1,2~тыс."],
        ),
        (
            1234.5678,
            ["1.2K", "1,2~k", "1234,6", "1,2~mil", "1,2~тыс."],
        ),
    ];
    for (v, want) in cases {
        for (l, w) in Locale::ALL.into_iter().zip(want) {
            assert_eq!(compact_in(l, v), k(w), "{} {v}", l.code());
        }
    }
    assert_eq!(compact_opt(None), "—");
    assert_eq!(compact_in(Locale::En, f64::NAN), "—");
}

#[test]
fn dates_match_intl() {
    let at = |y, m, d, h, mi, s| -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .and_then(|x| x.and_hms_opt(h, mi, s))
            .unwrap_or_default()
    };
    let a = at(2026, 10, 21, 17, 57, 3);
    let b = at(2026, 1, 5, 0, 7, 3);
    let want_a = [
        "10/21/2026, 5:57:03 PM",
        "21/10/2026 17:57:03",
        "21.10.2026, 17:57:03",
        "21/10/2026, 17:57:03",
        "21.10.2026, 17:57:03",
    ];
    let want_b = [
        "1/5/2026, 12:07:03 AM",
        "05/01/2026 0:07:03",
        "5.1.2026, 0:07:03",
        "5/1/2026, 0:07:03",
        "05.01.2026, 0:07:03",
    ];
    for (i, l) in Locale::ALL.into_iter().enumerate() {
        assert_eq!(date_time_in(l, &a), want_a[i]);
        assert_eq!(date_time_in(l, &b), want_b[i]);
    }
    let want = [
        "Oct 21, 2026",
        "21 oct. 2026",
        "21. Okt. 2026",
        "21 oct 2026",
        "21 окт. 2026 г.",
    ];
    for (l, w) in Locale::ALL.into_iter().zip(want) {
        assert_eq!(date_in(l, a.date()), w);
    }
    let may = at(2026, 5, 5, 0, 0, 0).date();
    let want = [
        "May 5, 2026",
        "5 mai 2026",
        "5. Mai 2026",
        "5 may 2026",
        "5 мая 2026 г.",
    ];
    for (l, w) in Locale::ALL.into_iter().zip(want) {
        assert_eq!(date_in(l, may), w);
    }
    assert_eq!(date_str("not a date"), "not a date");
}

#[test]
fn relative_times() {
    let now = 1_000_000_000;
    let r = |secs: i64| relative_in(Locale::En, now, now - secs);
    assert_eq!(r(5), "just now");
    assert_eq!(r(-30), "just now");
    assert_eq!(r(90), "1 minute ago");
    assert_eq!(
        r(600),
        fill(
            t_in(Locale::En, "shell", "timeMinutesAgo"),
            &[("count", &10)]
        )
    );
    assert_eq!(r(5_000), "1 hour ago");
    assert_eq!(
        r(3 * 3_600),
        fill(t_in(Locale::En, "shell", "timeHoursAgo"), &[("count", &3)])
    );
    assert_eq!(r(100_000), "1 day ago");
    assert_eq!(
        r(3 * 86_400),
        fill(t_in(Locale::En, "shell", "timeDaysAgo"), &[("count", &3)])
    );
    assert_eq!(r(700_000), "1 week ago");
    assert_eq!(
        r(3 * 604_800),
        fill(t_in(Locale::En, "shell", "timeWeeksAgo"), &[("count", &3)])
    );
    assert_eq!(r(3_000_000), "a long time ago");
    assert_eq!(
        relative_in(Locale::Fr, now, now - 600),
        fill(
            t_in(Locale::Fr, "shell", "timeMinutesAgo"),
            &[("count", &10)]
        )
    );
}

#[test]
fn units() {
    assert_eq!(bytes(512.0), "512 B");
    assert_eq!(bytes(1536.0), "1.5 KB");
    assert_eq!(bytes(1280.0), "1.3 KB");
    assert_eq!(bytes(150.0 * 1024.0 * 1024.0), "150 MB");
    assert_eq!(bytes(1.4 * 1024f64.powi(3)), "1.4 GB");
    assert_eq!(bytes(5.0 * 1024f64.powi(5)), "5120 TB");
    assert_eq!(duration(42), "<1m");
    assert_eq!(duration(125), "2m");
    assert_eq!(duration(2 * 3600 + 15 * 60 + 9), "2h 15m");
    assert_eq!(duration(-5), "<1m");
    assert!(distance_km([2.35, 48.85], [2.35, 48.85]).abs() < 1e-9);
    let d = distance_km([2.3522, 48.8566], [13.405, 52.52]);
    assert!((870.0..885.0).contains(&d), "{d}");
}
