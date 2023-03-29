use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

//Unsure about using the image path instead of its index to move it around
//Currently collections are always ordered the same way but that could change in the
//Future so in a way it's safer this way

///Callbacks are functions which the user can
///invoke under certain circumstances
#[derive(Clone, Debug)]
pub enum Callback {
    ReloadAll,
    Reload(Option<PathBuf>),
    Pop(Option<PathBuf>),
    NoAction,
}

impl Callback {
    pub fn from_callback(callback: Callback, path: Option<PathBuf>) -> Callback {
        match callback {
            Callback::ReloadAll => Self::ReloadAll,
            Callback::Reload(_) => Self::Reload(path),
            Callback::Pop(_) => Self::Pop(path),
            Callback::NoAction => Self::NoAction,
        }
    }
}

impl<'de> Deserialize<'de> for Callback {
    fn deserialize<D>(deserializer: D) -> Result<Callback, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        Ok(match s.as_str() {
            "ReloadAll" => Callback::ReloadAll,
            "Reload" => Callback::Reload(None),
            "Pop" => Callback::Pop(None),
            "" => Callback::NoAction,
            &_ => Callback::NoAction,
        })
    }
}

impl Serialize for Callback {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Callback::NoAction => "",
            Callback::ReloadAll => "ReloadAll",
            Callback::Pop(_) => "Pop",
            Callback::Reload(_) => "Reload",
        })
    }
}
