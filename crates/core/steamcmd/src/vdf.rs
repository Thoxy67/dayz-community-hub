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

    #[test]
    fn escapes_and_truncation() {
        let v = parse(r#""a" { "LaunchOptions" "say \"hi\" %command%" "b" { "c" "#);
        assert_eq!(
            v.str(&["a", "LaunchOptions"]),
            Some(r#"say "hi" %command%"#)
        );
    }
}
