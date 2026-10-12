//! VST3 plugin discovery and hosting primitives.
//!
//! Threading: everything that touches a plugin's controller, views, setup or state runs on the
//! UI thread (see [`main_thread`]); only `setProcessing` and `process` run on the thread that
//! holds a slot's processing lock.

pub mod host;
pub mod instance;
pub mod main_thread;
pub mod module;

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use vst3::Steinberg::TUID;

/// A VST3 class ID, shown and stored as 32 uppercase hex digits in the byte order the plugin
/// reports it (the same text as `moduleinfo.json`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassId(pub [u8; 16]);

impl ClassId {
    pub fn from_tuid(tuid: &TUID) -> ClassId {
        ClassId(tuid.map(|b| b as u8))
    }

    pub fn to_tuid(self) -> TUID {
        self.0.map(|b| b as i8)
    }

    pub fn parse(text: &str) -> Option<ClassId> {
        if text.len() != 32 || !text.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; 16];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
        }
        Some(ClassId(bytes))
    }
}

impl fmt::Display for ClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02X}"))
    }
}

impl fmt::Debug for ClassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClassId({self})")
    }
}

impl Serialize for ClassId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ClassId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<ClassId, D::Error> {
        let text = String::deserialize(d)?;
        ClassId::parse(&text).ok_or_else(|| serde::de::Error::custom("invalid class ID"))
    }
}

/// Reads a NUL-terminated 8-bit string field, replacing invalid UTF-8.
pub(crate) fn from_cstr(chars: &[std::ffi::c_char]) -> String {
    let bytes: Vec<u8> = chars
        .iter()
        .map(|&c| c as u8)
        .take_while(|&b| b != 0)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_id_round_trips_as_hex() {
        let id = ClassId([
            0x48, 0x44, 0x74, 0x47, 0x41, 0x49, 0x4E, 0x00, 0x8A, 0x5B, 0x5E, 0x5A, 0, 0, 0, 1,
        ]);
        let text = id.to_string();
        assert_eq!(text, "4844744741494E008A5B5E5A00000001");
        assert_eq!(ClassId::parse(&text), Some(id));
        assert_eq!(ClassId::parse(&text.to_lowercase()), Some(id));
        assert_eq!(ClassId::parse("12"), None);
        assert_eq!(ClassId::from_tuid(&id.to_tuid()), id);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(serde_json::from_str::<ClassId>(&json).unwrap(), id);
    }
}
