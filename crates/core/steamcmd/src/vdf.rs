//! Reading Steam's text VDF files (`config.vdf`, `localconfig.vdf`): quoted
//! keys, quoted values or `{ … }` blocks, `//` comments. Only what the
//! launcher reads from them: no writing, no binary VDF.

/// A value: a string, or a block of keyed values in file order.
#[derive(Debug, Clone, PartialEq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// The value under `path`, keys compared without case (Steam is not
    /// consistent: `Software` and `software` both occur).
    pub fn get(&self, path: &[&str]) -> Option<&Vdf> {
        let mut at = self;
        for key in path {
            let Vdf::Obj(items) = at else { return None };
            at = &items.iter().find(|(k, _)| k.eq_ignore_ascii_case(key))?.1;
        }
        Some(at)
    }

    pub fn str(&self, path: &[&str]) -> Option<&str> {
        match self.get(path)? {
            Vdf::Str(s) => Some(s),
            Vdf::Obj(_) => None,
        }
    }
}

/// Parse a whole file. Unbalanced or truncated input yields what was read.
pub fn parse(text: &str) -> Vdf {
    // Tokens are read as the parser asks for them and each string is moved
    // into the tree: `localconfig.vdf` runs to megabytes, and a token list
    // whose strings were then cloned into the tree held the file twice.
    Vdf::Obj(block(&mut Tokens(text.chars().peekable())))
}

/// The string under `path` in `text`, keys compared without case, read
/// without building the tree: the blocks off the path are skipped, and
/// nothing is copied but the answer. The same answer as
/// `parse(text).str(path)`, which `localconfig.vdf` (megabytes) would turn
/// into tens of thousands of strings for one value.
pub fn lookup(text: &str, path: &[&str]) -> Option<String> {
    let mut lex = Lexer { text, at: 0 };
    let mut depth = 0;
    loop {
        // Blocks are entered only along the path, so every key read here is
        // one of the block `path[..depth]` names.
        let key = match lex.next()? {
            Lex::Str(key) => key,
            // A stray brace, skipped as the parser does.
            Lex::Open => continue,
            // The block ends, and with it the search: the first block of a
            // name is the one that counts.
            Lex::Close => return None,
        };
        let hit = key.eq_ignore_ascii_case(path.get(depth)?);
        let last = depth + 1 == path.len();
        match lex.next()? {
            Lex::Str(value) if hit => return last.then(|| value.into_owned()),
            Lex::Str(_) => {}
            Lex::Open if hit && !last => depth += 1,
            Lex::Open if hit => return None,
            Lex::Open => lex.skip_block()?,
            // A key without a value closes its block.
            Lex::Close => return None,
        }
    }
}

enum Lex<'a> {
    Str(std::borrow::Cow<'a, str>),
    Open,
    Close,
}

/// [`Tokens`] over borrowed text: a string without escapes is a slice of it.
struct Lexer<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Lexer<'a> {
    fn next(&mut self) -> Option<Lex<'a>> {
        let b = self.text.as_bytes();
        while let Some(&c) = b.get(self.at) {
            self.at += 1;
            match c {
                b'{' => return Some(Lex::Open),
                b'}' => return Some(Lex::Close),
                b'/' if b.get(self.at) == Some(&b'/') => {
                    while b.get(self.at).is_some_and(|c| *c != b'\n') {
                        self.at += 1;
                    }
                }
                b'"' => return Some(Lex::Str(self.string())),
                _ => {}
            }
        }
        None
    }

    /// The rest of a string whose opening quote was just read.
    fn string(&mut self) -> std::borrow::Cow<'a, str> {
        let b = self.text.as_bytes();
        let start = self.at;
        // Quotes and backslashes are ASCII: every index cut at is a char
        // boundary.
        while let Some(&c) = b.get(self.at) {
            match c {
                b'"' => {
                    self.at += 1;
                    return self.text[start..self.at - 1].into();
                }
                b'\\' => break,
                _ => self.at += 1,
            }
        }
        if self.at >= b.len() {
            return self.text[start..].into();
        }
        // An escape: built the parser's way from here on.
        let mut s = self.text[start..self.at].to_string();
        let mut chars = self.text[self.at..].char_indices();
        let mut end = self.text.len();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some((_, 'n')) => s.push('\n'),
                    Some((_, 't')) => s.push('\t'),
                    Some((_, other)) => s.push(other),
                    None => break,
                },
                '"' => {
                    end = self.at + i + 1;
                    break;
                }
                other => s.push(other),
            }
        }
        self.at = end;
        s.into()
    }

    /// Skip to the end of the block just opened. `None` at the end of the
    /// text.
    fn skip_block(&mut self) -> Option<()> {
        let mut depth = 1usize;
        while depth > 0 {
            match self.next()? {
                Lex::Open => depth += 1,
                Lex::Close => depth -= 1,
                Lex::Str(_) => {}
            }
        }
        Some(())
    }
}

#[derive(Debug)]
enum Tok {
    Str(String),
    Open,
    Close,
}

struct Tokens<'a>(std::iter::Peekable<std::str::Chars<'a>>);

impl Iterator for Tokens<'_> {
    type Item = Tok;

    fn next(&mut self) -> Option<Tok> {
        let chars = &mut self.0;
        while let Some(c) = chars.next() {
            match c {
                '{' => return Some(Tok::Open),
                '}' => return Some(Tok::Close),
                '/' if chars.peek() == Some(&'/') => {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                '"' => {
                    let mut s = String::new();
                    while let Some(c) = chars.next() {
                        match c {
                            '\\' => match chars.next() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some(other) => s.push(other),
                                None => break,
                            },
                            '"' => break,
                            other => s.push(other),
                        }
                    }
                    return Some(Tok::Str(s));
                }
                _ => {}
            }
        }
        None
    }
}

fn block(tokens: &mut Tokens<'_>) -> Vec<(String, Vdf)> {
    let mut items = Vec::new();
    while let Some(tok) = tokens.next() {
        match tok {
            Tok::Close => break,
            Tok::Open => {} // a stray brace: skip it
            Tok::Str(key) => match tokens.next() {
                Some(Tok::Str(v)) => items.push((key, Vdf::Str(v))),
                Some(Tok::Open) => {
                    let inner = block(tokens);
                    items.push((key, Vdf::Obj(inner)));
                }
                // A key without a value closes its block.
                Some(Tok::Close) | None => break,
            },
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_blocks_and_values() {
        let v = parse(
            r#"
"InstallConfigStore"
{
	"Software" { "Valve" { "Steam" {
		// the tools each game runs under
		"CompatToolMapping"
		{
			"221100" { "name" "proton-cachyos-native" "priority" "250" }
		}
	} } }
}
"#,
        );
        assert_eq!(
            v.str(&[
                "InstallConfigStore",
                "software",
                "valve",
                "steam",
                "CompatToolMapping",
                "221100",
                "name"
            ]),
            Some("proton-cachyos-native")
        );
        assert_eq!(v.str(&["nope"]), None);
    }

    #[test]
    fn a_key_without_a_value_ends_its_block() {
        let v = parse(r#""a" { "k" } "b" "c" "#);
        assert_eq!(v.get(&["a"]), Some(&Vdf::Obj(Vec::new())));
        assert_eq!(v.str(&["b"]), Some("c"));
    }

    /// `lookup` answers what the tree does, on the shapes the tests above use
    /// and on the files' oddities.
    #[test]
    fn lookup_agrees_with_the_tree() {
        let texts = [
            r#""InstallConfigStore" { "Software" { "Valve" { "Steam" { // tools
                "CompatToolMapping" { "0" { "name" "proton_9" } "221100" { "name" "GE-Proton9-20" } }
            } } } }"#,
            r#""a" { "k" } "b" "c" "#,
            r#""a" { "LaunchOptions" "say \"hi\" %command%" "b" { "c" "#,
            r#""a" { "x" { "LaunchOptions" "no" } "launchoptions" "yes\tthere" } "a" { "z" "1" }"#,
            r#"} "a" { "b" "c" }"#,
            r#"{ "a" { "b" "é{}\"" } }"#,
            r#""a" { "b" { } } "a" "s""#,
            "",
        ];
        let paths: &[&[&str]] = &[
            &[
                "InstallConfigStore",
                "software",
                "valve",
                "steam",
                "CompatToolMapping",
                "221100",
                "name",
            ],
            &[
                "InstallConfigStore",
                "Software",
                "Valve",
                "Steam",
                "CompatToolMapping",
                "0",
                "name",
            ],
            &[
                "InstallConfigStore",
                "Software",
                "Valve",
                "Steam",
                "CompatToolMapping",
                "1",
                "name",
            ],
            &["InstallConfigStore", "Software"],
            &["a"],
            &["a", "k"],
            &["a", "b"],
            &["a", "z"],
            &["a", "LaunchOptions"],
            &["a", "x", "LaunchOptions"],
            &["b"],
            &[],
        ];
        for text in texts {
            let tree = parse(text);
            for path in paths {
                assert_eq!(
                    lookup(text, path).as_deref(),
                    tree.str(path),
                    "{path:?} in {text}"
                );
            }
        }
    }

    #[test]
    fn escapes_and_truncation() {
        let v = parse(r#""a" { "LaunchOptions" "say \"hi\" %command%" "b" { "c" "#);
        assert_eq!(
            v.str(&["a", "LaunchOptions"]),
            Some(r#"say "hi" %command%"#)
        );
    }
}
