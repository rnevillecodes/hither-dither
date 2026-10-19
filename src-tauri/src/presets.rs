//! Effect presets: named groups of effects, with each plugin's settings, kept by the app.
//!
//! The library lives in `effect-presets.json` in the app data directory:
//!
//! ```text
//! { "format": "hitherdither-presets", "version": 1, "nextId": 3,
//!   "presets": [ { "id": 1, "name": "Vocal cleanup", "effects": [StoredEffect…] } ] }
//! ```
//!
//! Every change rewrites the whole file through a temporary file. A file that can't be read
//! opens as an empty library and is left alone until the next successful change replaces it.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::fx::layout::{MAX_NAME, valid_name};
use crate::plugins::cache::write_json;
use crate::session::StoredEffect;

pub const PRESETS_FILE: &str = "effect-presets.json";
const FORMAT: &str = "hitherdither-presets";
const VERSION: u32 = 1;

/// Identifies a preset; kept when it is renamed.
pub type PresetId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: PresetId,
    pub name: String,
    pub effects: Vec<StoredEffect>,
}

/// A preset as listed: no plugin settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub id: PresetId,
    pub name: String,
    pub effect_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PresetError {
    /// Another preset has the name, ignoring case, and replacing wasn't asked for.
    NameTaken,
    Failed(String),
}

impl From<String> for PresetError {
    fn from(e: String) -> Self {
        PresetError::Failed(e)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PresetFile {
    format: String,
    version: u32,
    next_id: PresetId,
    presets: Vec<Preset>,
}

#[derive(Debug, Clone)]
struct Inner {
    next_id: PresetId,
    presets: Vec<Preset>,
}

/// The app's presets, with every change written to disk before it takes effect.
pub struct PresetLibrary {
    path: PathBuf,
    inner: Mutex<Inner>,
}

const NOT_FOUND: &str = "That preset no longer exists";
const UNREADABLE: &str = "Your effect presets could not be read, so none are listed. The file \
                          is kept until you next save a preset, which replaces it.";

fn key(name: &str) -> String {
    name.to_lowercase()
}

fn checked_name(name: &str) -> Result<String, PresetError> {
    valid_name(name).ok_or_else(|| {
        PresetError::Failed(format!("A preset name must be 1 to {MAX_NAME} characters"))
    })
}

fn read(path: &Path) -> Result<Option<Inner>, ()> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    let file: PresetFile = serde_json::from_slice(&bytes).map_err(|_| ())?;
    if file.format != FORMAT || file.version != VERSION {
        return Err(());
    }
    let next_id = file
        .presets
        .iter()
        .map(|p| p.id + 1)
        .max()
        .unwrap_or(1)
        .max(file.next_id);
    Ok(Some(Inner {
        next_id,
        presets: file.presets,
    }))
}

impl PresetLibrary {
    /// Opens the library in `dir` (the app data directory). Returns a notice to show the
    /// user when the file exists but can't be read; the library is then empty.
    pub fn open(dir: &Path) -> (PresetLibrary, Option<String>) {
        let path = dir.join(PRESETS_FILE);
        let (inner, notice) = match read(&path) {
            Ok(inner) => (inner, None),
            Err(()) => (None, Some(UNREADABLE.to_string())),
        };
        let inner = inner.unwrap_or(Inner {
            next_id: 1,
            presets: Vec::new(),
        });
        let library = PresetLibrary {
            path,
            inner: Mutex::new(inner),
        };
        (library, notice)
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The presets, sorted by name ignoring case.
    pub fn list(&self) -> Vec<PresetInfo> {
        let mut list: Vec<PresetInfo> = self
            .lock()
            .presets
            .iter()
            .map(|p| PresetInfo {
                id: p.id,
                name: p.name.clone(),
                effect_count: p.effects.len(),
            })
            .collect();
        list.sort_by(|a, b| key(&a.name).cmp(&key(&b.name)).then(a.id.cmp(&b.id)));
        list
    }

    pub fn get(&self, id: PresetId) -> Option<Preset> {
        self.lock().presets.iter().find(|p| p.id == id).cloned()
    }

    /// Saves `effects` as preset `name`. When another preset has that name, ignoring case,
    /// fails with [`PresetError::NameTaken`] unless `replace`, which overwrites it.
    pub fn save(
        &self,
        name: &str,
        effects: Vec<StoredEffect>,
        replace: bool,
    ) -> Result<PresetId, PresetError> {
        let name = checked_name(name)?;
        self.change(|inner| {
            let taken = inner
                .presets
                .iter()
                .position(|p| key(&p.name) == key(&name));
            match taken {
                Some(_) if !replace => Err(PresetError::NameTaken),
                Some(i) => {
                    let preset = &mut inner.presets[i];
                    preset.name = name;
                    preset.effects = effects;
                    Ok(preset.id)
                }
                None => {
                    let id = inner.next_id;
                    inner.next_id += 1;
                    inner.presets.push(Preset { id, name, effects });
                    Ok(id)
                }
            }
        })
    }

    /// Renames preset `id`. When another preset has the name, ignoring case, fails with
    /// [`PresetError::NameTaken`] unless `replace`, which deletes that other preset.
    pub fn rename(&self, id: PresetId, name: &str, replace: bool) -> Result<(), PresetError> {
        let name = checked_name(name)?;
        self.change(|inner| {
            if !inner.presets.iter().any(|p| p.id == id) {
                return Err(PresetError::Failed(NOT_FOUND.into()));
            }
            let other = |p: &Preset| p.id != id && key(&p.name) == key(&name);
            if !replace && inner.presets.iter().any(other) {
                return Err(PresetError::NameTaken);
            }
            inner.presets.retain(|p| !other(p));
            let preset = inner.presets.iter_mut().find(|p| p.id == id).unwrap();
            preset.name = name;
            Ok(())
        })
    }

    pub fn delete(&self, id: PresetId) -> Result<(), String> {
        self.change(|inner| {
            let i = inner
                .presets
                .iter()
                .position(|p| p.id == id)
                .ok_or_else(|| PresetError::Failed(NOT_FOUND.into()))?;
            inner.presets.remove(i);
            Ok(())
        })
        .map_err(|e| match e {
            PresetError::Failed(e) => e,
            PresetError::NameTaken => unreachable!("deleting takes no name"),
        })
    }

    /// Applies `edit` to a copy of the library, writes it, and only then keeps it.
    fn change<T>(
        &self,
        edit: impl FnOnce(&mut Inner) -> Result<T, PresetError>,
    ) -> Result<T, PresetError> {
        let mut inner = self.lock();
        let mut next = inner.clone();
        let result = edit(&mut next)?;
        let file = PresetFile {
            format: FORMAT.into(),
            version: VERSION,
            next_id: next.next_id,
            presets: next.presets.clone(),
        };
        write_json(&self.path, &file)
            .map_err(|e| format!("The presets could not be saved: {e}"))?;
        *inner = next;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::ClassId;
    use crate::session::StoredState;

    fn effect(name: &str, bypassed: bool) -> StoredEffect {
        StoredEffect {
            class_id: ClassId([7; 16]),
            name: name.into(),
            vendor: "HitherDither".into(),
            version: "1.0.0".into(),
            bypassed,
            state: StoredState {
                component: vec![1, 2, 3],
                controller: vec![4],
            },
            pending_params: vec![(0, 0.5)],
        }
    }

    fn names(library: &PresetLibrary) -> Vec<String> {
        library.list().into_iter().map(|p| p.name).collect()
    }

    #[test]
    fn saves_and_reopens_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let (library, notice) = PresetLibrary::open(dir.path());
        assert_eq!(notice, None);
        let effects = vec![effect("EQ", false), effect("Comp", true)];
        let id = library
            .save("  Vocal cleanup ", effects.clone(), false)
            .unwrap();

        let (reopened, notice) = PresetLibrary::open(dir.path());
        assert_eq!(notice, None);
        let preset = reopened.get(id).unwrap();
        assert_eq!(preset.name, "Vocal cleanup");
        assert_eq!(preset.effects, effects);
        let list = reopened.list();
        assert_eq!(list[0].effect_count, 2);
        // Ids aren't reused after reopening.
        let next = reopened.save("Other", Vec::new(), false).unwrap();
        assert!(next > id);
    }

    #[test]
    fn lists_by_name_ignoring_case() {
        let dir = tempfile::tempdir().unwrap();
        let (library, _) = PresetLibrary::open(dir.path());
        for name in ["vocal", "Master bus", "Ägir", "drums"] {
            library.save(name, Vec::new(), false).unwrap();
        }
        assert_eq!(names(&library), ["drums", "Master bus", "vocal", "Ägir"]);
    }

    #[test]
    fn a_taken_name_is_replaced_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let (library, _) = PresetLibrary::open(dir.path());
        let id = library
            .save("Master bus", vec![effect("A", false)], false)
            .unwrap();
        let taken = library.save("master BUS", vec![effect("B", false)], false);
        assert_eq!(taken, Err(PresetError::NameTaken));
        assert_eq!(library.get(id).unwrap().effects[0].name, "A");

        let replaced = library
            .save("master bus", vec![effect("B", false)], true)
            .unwrap();
        assert_eq!(replaced, id);
        assert_eq!(names(&library), ["master bus"]);
        assert_eq!(library.get(id).unwrap().effects[0].name, "B");
    }

    #[test]
    fn rejects_invalid_names() {
        let dir = tempfile::tempdir().unwrap();
        let (library, _) = PresetLibrary::open(dir.path());
        assert!(matches!(
            library.save("   ", Vec::new(), false),
            Err(PresetError::Failed(_))
        ));
        assert!(library.save(&"x".repeat(65), Vec::new(), false).is_err());
        assert!(library.save(&"x".repeat(64), Vec::new(), false).is_ok());
    }

    #[test]
    fn rename_keeps_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let (library, _) = PresetLibrary::open(dir.path());
        let a = library.save("A", vec![effect("EQ", false)], false).unwrap();
        let b = library.save("B", Vec::new(), false).unwrap();

        library.rename(a, "Renamed", false).unwrap();
        assert_eq!(library.get(a).unwrap().name, "Renamed");
        assert_eq!(library.get(a).unwrap().effects.len(), 1);
        // Its own name, in another case, isn't taken.
        library.rename(a, "RENAMED", false).unwrap();

        assert_eq!(library.rename(a, "b", false), Err(PresetError::NameTaken));
        library.rename(a, "b", true).unwrap();
        assert_eq!(library.get(b), None);
        assert_eq!(names(&library), ["b"]);
        assert_eq!(library.list()[0].id, a);
    }

    #[test]
    fn deletes() {
        let dir = tempfile::tempdir().unwrap();
        let (library, _) = PresetLibrary::open(dir.path());
        let a = library.save("A", Vec::new(), false).unwrap();
        library.save("B", Vec::new(), false).unwrap();
        library.delete(a).unwrap();
        assert_eq!(names(&library), ["B"]);
        assert!(library.delete(a).is_err());
        let (reopened, _) = PresetLibrary::open(dir.path());
        assert_eq!(names(&reopened), ["B"]);
    }

    #[test]
    fn an_unreadable_file_opens_empty_and_is_kept_until_the_next_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(PRESETS_FILE);
        std::fs::write(&path, b"{ not json").unwrap();

        let (library, notice) = PresetLibrary::open(dir.path());
        assert!(notice.is_some());
        assert!(library.list().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), b"{ not json");

        library.save("A", Vec::new(), false).unwrap();
        let (reopened, notice) = PresetLibrary::open(dir.path());
        assert_eq!(notice, None);
        assert_eq!(names(&reopened), ["A"]);
    }

    #[test]
    fn another_format_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let file =
            r#"{ "format": "hitherdither-presets", "version": 2, "nextId": 1, "presets": [] }"#;
        std::fs::write(dir.path().join(PRESETS_FILE), file).unwrap();
        assert!(PresetLibrary::open(dir.path()).1.is_some());
    }

    #[test]
    fn a_failed_write_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (library, _) = PresetLibrary::open(dir.path());
        let a = library.save("A", Vec::new(), false).unwrap();
        // The temporary file can't be written while a directory has its name.
        std::fs::create_dir(dir.path().join("effect-presets.json.tmp")).unwrap();

        assert!(matches!(
            library.save("B", Vec::new(), false),
            Err(PresetError::Failed(_))
        ));
        assert!(library.rename(a, "C", false).is_err());
        assert!(library.delete(a).is_err());
        assert_eq!(names(&library), ["A"]);
    }
}
