//! A minimal reader for Valve's text KeyValues format, which is what
//! `libraryfolders.vdf` and `appmanifest_*.acf` are written in.
//!
//! We only need a handful of values out of known shapes, so rather than
//! building a tree this walks the token stream and records every
//! `"key" "value"` pair it meets, whatever its nesting. That answers both "every
//! library `path`" and "this manifest's `name`", and unfamiliar keys and blocks
//! simply flow past.

/// One scalar entry: a key and its value.
#[derive(Debug, Clone)]
pub struct Entry {
    pub key: String,
    pub value: String,
}

/// Parses the scalar entries out of a KeyValues document.
pub fn parse(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut tokens = Tokenizer {
        chars: text.chars().peekable(),
    };
    // A quoted string is only a key until we learn what follows it: another
    // string makes it a key/value pair, a brace makes it a block name.
    let mut pending_key: Option<String> = None;

    while let Some(token) = tokens.next_token() {
        match token {
            Token::OpenBrace | Token::CloseBrace => pending_key = None,
            Token::Str(text) => match pending_key.take() {
                Some(key) => entries.push(Entry { key, value: text }),
                None => pending_key = Some(text),
            },
        }
    }
    entries
}

/// The first value recorded for `key`.
pub fn first(entries: &[Entry], key: &str) -> Option<String> {
    entries
        .iter()
        .find(|e| e.key.eq_ignore_ascii_case(key))
        .map(|e| e.value.clone())
}

/// Every value for `key`, in document order.
pub fn all(entries: &[Entry], key: &str) -> Vec<String> {
    entries
        .iter()
        .filter(|e| e.key.eq_ignore_ascii_case(key))
        .map(|e| e.value.clone())
        .collect()
}

enum Token {
    OpenBrace,
    CloseBrace,
    Str(String),
}

struct Tokenizer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl Tokenizer<'_> {
    fn next_token(&mut self) -> Option<Token> {
        loop {
            let c = *self.chars.peek()?;
            if c.is_whitespace() {
                self.chars.next();
                continue;
            }
            match c {
                '{' => {
                    self.chars.next();
                    return Some(Token::OpenBrace);
                }
                '}' => {
                    self.chars.next();
                    return Some(Token::CloseBrace);
                }
                '"' => {
                    self.chars.next();
                    return Some(Token::Str(self.quoted()));
                }
                '/' => {
                    // `//` starts a comment; anything else is a stray character.
                    self.chars.next();
                    if self.chars.peek() == Some(&'/') {
                        for c in self.chars.by_ref() {
                            if c == '\n' {
                                break;
                            }
                        }
                    }
                    continue;
                }
                _ => {
                    // Unquoted token: read to the next delimiter.
                    let mut text = String::new();
                    while let Some(&c) = self.chars.peek() {
                        if c.is_whitespace() || c == '{' || c == '}' || c == '"' {
                            break;
                        }
                        text.push(c);
                        self.chars.next();
                    }
                    if text.is_empty() {
                        self.chars.next();
                        continue;
                    }
                    return Some(Token::Str(text));
                }
            }
        }
    }

    /// Reads the body of a quoted string, honouring backslash escapes.
    fn quoted(&mut self) -> String {
        let mut out = String::new();
        while let Some(c) = self.chars.next() {
            match c {
                '"' => break,
                '\\' => match self.chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => break,
                },
                other => out.push(other),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARY_FOLDERS: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"730"		"73976882775"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
		}
	}
}
"#;

    #[test]
    fn reads_every_library_path() {
        let entries = parse(LIBRARY_FOLDERS);
        assert_eq!(
            all(&entries, "path"),
            vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]
        );
    }

    #[test]
    fn block_names_are_not_mistaken_for_values() {
        let entries = parse(LIBRARY_FOLDERS);
        // "libraryfolders" and "0" name blocks, so they are never keys.
        assert!(first(&entries, "libraryfolders").is_none());
        assert!(first(&entries, "0").is_none());
        // The app id inside the apps block is a real key/value pair.
        assert_eq!(first(&entries, "730").as_deref(), Some("73976882775"));
    }

    #[test]
    fn reads_a_flat_app_manifest() {
        let manifest = r#"
"AppState"
{
	"appid"		"1912410"
	"name"		"Minecraft Dungeons II"
	"StateFlags"		"4"
	"installdir"		"Minecraft Dungeons II"
	"SizeOnDisk"		"10102996780"
}
"#;
        let entries = parse(manifest);
        assert_eq!(first(&entries, "appid").as_deref(), Some("1912410"));
        assert_eq!(
            first(&entries, "name").as_deref(),
            Some("Minecraft Dungeons II")
        );
        assert_eq!(
            first(&entries, "installdir").as_deref(),
            Some("Minecraft Dungeons II")
        );
    }

    #[test]
    fn skips_comments_and_tolerates_unquoted_tokens() {
        let entries = parse("// a comment\n\"root\"\n{\n\tkey value\n}\n");
        assert_eq!(first(&entries, "key").as_deref(), Some("value"));
    }

    #[test]
    fn an_empty_document_yields_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("   \n\t ").is_empty());
    }
}
