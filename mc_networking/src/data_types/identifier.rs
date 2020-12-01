use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref};

const ALLOWED_CHARACTERS: &str = "0123456789abcdefghijklmnopqrstuvwxyz-_";

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Identifier<T: Deref<Target = str> = String> {
    text: T,
    name_pos: usize,
}
impl<T: Deref<Target = str>> Identifier<T> {
    /// Created a new identifier from text
    /// will panic if the text doesn't contain a namespace (i.e. minecraft:*)
    /// use Identifier::from if you want to use the default minecraft:namespace
    pub fn new(text: T) -> Self {
        Self {
            name_pos: text.find(':').expect("invalid identifier") + 1,
            text,
        }
    }

    pub fn namespace(&self) -> &str {
        &self.text[0..self.name_pos - 1]
    }
    pub fn name(&self) -> &str {
        &self.text[self.name_pos..self.text.len()]
    }
}

impl<T: Deref<Target = str>> fmt::Debug for Identifier<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Identifier")
            .field(&self.text.deref())
            .finish()
    }
}

impl<'a> From<&'a str> for Identifier<String> {
    fn from(text: &'a str) -> Self {
        let this = if text.contains(':') {
            let name_pos = text.find(':').unwrap() + 1;
            Self {
                text: text.to_string(),
                name_pos,
            }
        }
        else {
            Self {
                text: String::from("minecraft:") + text,
                name_pos: 10,
            }
        };
        debug_assert!(
            {
                this.namespace()
                    .chars()
                    .all(|c| ALLOWED_CHARACTERS.chars().any(|c2| c == c2))
            },
            "invalid namespace"
        );
        debug_assert!(
            {
                this.name()
                    .chars()
                    .all(|c| ALLOWED_CHARACTERS.chars().any(|c2| c == c2))
            },
            "invalid name"
        );
        this
    }
}

impl ToString for Identifier {
    fn to_string(&self) -> String {
        self.namespace().to_owned() + ":" + self.name()
    }
}

impl Deref for Identifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}
