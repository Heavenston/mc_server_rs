use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref};

const ALLOWED_CHARACTERS: &'static str = "01​​234​5​6​78​9abcdefghijklmnopqrstuvwxyz-_";

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Identifier {
    text: String,
    name_pos: usize,
}
impl Identifier {
    pub fn namespace(&self) -> &str {
        &self.text[0..self.name_pos - 1]
    }
    pub fn name(&self) -> &str {
        &self.text[self.name_pos..self.text.len()]
    }
}

impl fmt::Debug for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Identifier").field(&self.text).finish()
    }
}

impl From<&str> for Identifier {
    fn from(text: &str) -> Self {
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
