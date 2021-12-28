use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
struct NBTMapEntry<T: Serialize> {
    name: String,
    id: i32,
    element: T,
}

/// This is a format used in the current network protocol,
/// most notably used in the JoinGame packet.
#[derive(Clone, Debug, Serialize)]
pub struct NBTMap<T: Serialize> {
    #[serde(rename = "type")]
    self_type: String,
    value: Vec<NBTMapEntry<T>>,
}

impl<T: Serialize> NBTMap<T> {
    pub fn new(self_type: String) -> NBTMap<T> {
        NBTMap {
            self_type,
            value: Vec::new(),
        }
    }

    pub fn push_element(&mut self, name: String, element: T) {
        let id = self.value.len() as i32;
        self.value.push(NBTMapEntry { id, name, element });
    }
}

#[macro_export]
macro_rules! map(
    { $($key:expr => $value:expr),+ } => {
        {
            let mut m = ::std::collections::HashMap::default();
            $(
                m.insert($key, $value);
            )+
            m
        }
     };
);
