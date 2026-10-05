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
    let tokens = tokenize(text);
    let mut i = 0;
    Vdf::Obj(block(&tokens, &mut i))
}

#[derive(Debug)]
enum Tok {
    Str(String),
    Open,
    Close,
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
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
                out.push(Tok::Str(s));
            }
            _ => {}
        }
    }
    out
}

fn block(tokens: &[Tok], i: &mut usize) -> Vec<(String, Vdf)> {
    let mut items = Vec::new();
    while *i < tokens.len() {
        match &tokens[*i] {
            Tok::Close => {
                *i += 1;
                break;
            }
            Tok::Open => *i += 1, // a stray brace: skip it
            Tok::Str(key) => {
                *i += 1;
                match tokens.get(*i) {
                    Some(Tok::Str(v)) => {
                        items.push((key.clone(), Vdf::Str(v.clone())));
                        *i += 1;
                    }
                    Some(Tok::Open) => {
                        *i += 1;
                        let inner = block(tokens, i);
                        items.push((key.clone(), Vdf::Obj(inner)));
                    }
                    _ => {}
                }
            }
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
    fn escapes_and_truncation() {
        let v = parse(r#""a" { "LaunchOptions" "say \"hi\" %command%" "b" { "c" "#);
        assert_eq!(
            v.str(&["a", "LaunchOptions"]),
            Some(r#"say "hi" %command%"#)
        );
    }
}
