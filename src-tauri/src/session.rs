//! Session files: one tab's WAV reference, unsaved edits and effect chain, as versioned
//! JSON (`.hdsession.json`). The WAV itself is never written; a session only points to it.
//!
//! Stored edits are plain mirrors of [`EditList`] and are rebuilt through
//! [`EditList::from_parts`], so a hand-edited or damaged file can't produce a list that
//! misplays. Plugin state is the plugin's own `getState` bytes, in base64.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::edit::{ClipFade, Crossfade, Curve, Dir, EditList, Ramp, Segment};
use crate::plugins::ClassId;
use crate::plugins::instance::State;

/// The `format` field of every session file.
pub const FORMAT: &str = "hitherdither-session";
/// Newest format version this build reads and the one it writes.
pub const VERSION: u32 = 1;
/// Session file names end with this (compared ignoring case).
pub const EXTENSION: &str = ".hdsession.json";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SessionError {
    #[error("“{0}” is not a valid session file.")]
    Invalid(String),
    #[error("“{0}” was made by a newer version of HitherDither.")]
    Newer(String),
    #[error("The session's audio file “{0}” could not be found.")]
    MissingAudio(String),
    #[error("Could not read “{name}”: {reason}")]
    Read { name: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFile {
    pub format: String,
    pub version: u32,
    pub audio: AudioRef,
    pub edits: StoredEdits,
    pub chain: StoredChain,
}

/// The WAV a session belongs to, and what it was like when the session was saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRef {
    /// Absolute, as the file was opened.
    pub path: PathBuf,
    /// From the session file's folder, `/`-separated; `None` when there is no relative
    /// path (another drive).
    pub relative_path: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub channel_mask: Option<u32>,
    /// "PCM" or "IEEE float".
    pub encoding: String,
    pub bit_depth: u16,
    pub valid_bits: u16,
    /// Source frames as loaded.
    pub frames: u64,
    pub size_bytes: u64,
    /// Modification time in milliseconds since the Unix epoch.
    pub modified: Option<u64>,
}

impl AudioRef {
    /// Same sample rate, channels and layout, sample format and length: the session's
    /// edits and effects fit this audio.
    pub fn format_matches(&self, other: &AudioRef) -> bool {
        (
            self.sample_rate,
            self.channels,
            self.channel_mask,
            &self.encoding,
            self.bit_depth,
            self.valid_bits,
            self.frames,
        ) == (
            other.sample_rate,
            other.channels,
            other.channel_mask,
            &other.encoding,
            other.bit_depth,
            other.valid_bits,
            other.frames,
        )
    }

    /// Same size and modification time: the file is very likely untouched.
    pub fn stamp_matches(&self, other: &AudioRef) -> bool {
        self.size_bytes == other.size_bytes && self.modified == other.modified
    }

    /// The WAV's file name, for messages.
    pub fn name(&self) -> String {
        file_name(&self.path)
    }
}

/// [`EditList`] as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEdits {
    pub source_frames: u64,
    pub segments: Vec<StoredSegment>,
    pub trim: [u64; 2],
    pub fade_in: ClipFade,
    pub fade_out: ClipFade,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSegment {
    pub src_start: u64,
    pub len: u64,
    pub silent: bool,
    pub ramps: Vec<StoredRamp>,
    pub xfade: Option<Crossfade>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StoredRamp {
    pub t0: f64,
    pub t1: f64,
    pub dir: Dir,
    pub curve: Curve,
}

impl From<&EditList> for StoredEdits {
    fn from(list: &EditList) -> Self {
        let trim = list.trim();
        StoredEdits {
            source_frames: list.source_frames(),
            segments: list
                .segments()
                .iter()
                .map(|s| StoredSegment {
                    src_start: s.src_start,
                    len: s.len,
                    silent: s.silent,
                    ramps: s
                        .ramps
                        .iter()
                        .map(|r| StoredRamp {
                            t0: r.t0,
                            t1: r.t1,
                            dir: r.dir,
                            curve: r.curve,
                        })
                        .collect(),
                    xfade: s.xfade,
                })
                .collect(),
            trim: [trim.start, trim.end],
            fade_in: list.fade_in(),
            fade_out: list.fade_out(),
        }
    }
}

impl StoredEdits {
    /// The edit list, or `None` if the stored edits are inconsistent.
    pub fn to_edit_list(&self) -> Option<EditList> {
        let segments = self
            .segments
            .iter()
            .map(|s| Segment {
                src_start: s.src_start,
                len: s.len,
                silent: s.silent,
                ramps: s
                    .ramps
                    .iter()
                    .map(|r| Ramp {
                        t0: r.t0,
                        t1: r.t1,
                        dir: r.dir,
                        curve: r.curve,
                    })
                    .collect(),
                xfade: s.xfade,
            })
            .collect();
        let [start, end] = self.trim;
        EditList::from_parts(
            self.source_frames,
            segments,
            start..end,
            self.fade_in,
            self.fade_out,
        )
        .ok()
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredChain {
    pub bypass_all: bool,
    pub effects: Vec<StoredEffect>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEffect {
    pub class_id: ClassId,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub bypassed: bool,
    pub state: StoredState,
    /// Parameter changes the processor hadn't received yet when the session was saved,
    /// as `(param ID, normalized value)`.
    pub pending_params: Vec<(u32, f64)>,
}

/// A plugin's `getState` bytes, base64 in the file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StoredState {
    #[serde(with = "b64")]
    pub component: Vec<u8>,
    #[serde(with = "b64")]
    pub controller: Vec<u8>,
}

impl From<State> for StoredState {
    fn from(s: State) -> Self {
        StoredState {
            component: s.component,
            controller: s.controller,
        }
    }
}

impl From<&StoredState> for State {
    fn from(s: &StoredState) -> Self {
        State {
            component: s.component.clone(),
            controller: s.controller.clone(),
        }
    }
}

mod b64 {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD.decode(text).map_err(serde::de::Error::custom)
    }
}

/// Just the fields that say what a file is, read before the rest so a newer version
/// is reported as such even if its layout changed.
#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
}

impl SessionFile {
    pub fn new(audio: AudioRef, edits: &EditList, chain: StoredChain) -> Self {
        SessionFile {
            format: FORMAT.to_string(),
            version: VERSION,
            audio,
            edits: edits.into(),
            chain,
        }
    }

    /// Parses and validates a session; `name` is the file's name, for messages.
    pub fn parse(text: &str, name: &str) -> Result<SessionFile, SessionError> {
        let invalid = || SessionError::Invalid(name.to_string());
        let header: Header = serde_json::from_str(text).map_err(|_| invalid())?;
        if header.format != FORMAT {
            return Err(invalid());
        }
        if header.version > VERSION {
            return Err(SessionError::Newer(name.to_string()));
        }
        let file: SessionFile = serde_json::from_str(text).map_err(|_| invalid())?;
        if file.edits.source_frames != file.audio.frames || file.edits.to_edit_list().is_none() {
            return Err(invalid());
        }
        Ok(file)
    }

    /// Reads and validates the session at `path`.
    pub fn read(path: &Path) -> Result<SessionFile, SessionError> {
        let name = file_name(path);
        let text = std::fs::read_to_string(path).map_err(|e| match e.kind() {
            // Not UTF-8: not something this app wrote.
            std::io::ErrorKind::InvalidData => SessionError::Invalid(name.clone()),
            _ => SessionError::Read {
                name: name.clone(),
                reason: e.to_string(),
            },
        })?;
        Self::parse(&text, &name)
    }

    /// The edits, already validated by [`parse`](Self::parse).
    pub fn edit_list(&self) -> EditList {
        self.edits.to_edit_list().expect("validated when parsed")
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("sessions always serialize")
    }

    /// The WAV this session refers to, as saved from `session_path`: the absolute path if
    /// that file exists, otherwise the path relative to the session's folder.
    pub fn resolve_audio(&self, session_path: &Path) -> Result<PathBuf, SessionError> {
        if self.audio.path.is_file() {
            return Ok(self.audio.path.clone());
        }
        if let (Some(rel), Some(dir)) = (&self.audio.relative_path, session_path.parent()) {
            let mut path = dir.to_path_buf();
            path.extend(rel.split('/').filter(|c| !c.is_empty()));
            if path.is_file() {
                return Ok(path);
            }
        }
        Err(SessionError::MissingAudio(self.audio.name()))
    }
}

/// A session's WAV, as `read_session` reports it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// The WAV to open for the session: where it was, or next to the session if moved.
    /// `None` when neither exists, so the user can choose another.
    pub audio_path: Option<PathBuf>,
    /// The recorded WAV's file name, for messages.
    pub audio_name: String,
}

/// Checks the session at `path` and finds its WAV. Fails only if the session itself can't
/// be read; a missing WAV is `audio_path: None`.
pub fn read_info(path: &Path) -> Result<SessionInfo, SessionError> {
    let session = SessionFile::read(path)?;
    Ok(SessionInfo {
        audio_path: session.resolve_audio(path).ok(),
        audio_name: session.audio.name(),
    })
}

/// Whether `path` names a session file.
pub fn is_session_path(path: &Path) -> bool {
    let name = file_name(path).to_lowercase();
    name.len() > EXTENSION.len() && name.ends_with(EXTENSION)
}

/// `to` relative to the folder `from_dir`, `/`-separated, worked out from the paths alone.
/// `None` when they have no common root (another drive) or either isn't absolute.
pub fn relative_path(from_dir: &Path, to: &Path) -> Option<String> {
    if !from_dir.is_absolute() || !to.is_absolute() {
        return None;
    }
    let base: Vec<Component> = from_dir.components().collect();
    let target: Vec<Component> = to.components().collect();
    let common = base
        .iter()
        .zip(&target)
        .take_while(|(a, b)| same_component(a, b))
        .count();
    // Not even the root (drive or prefix) in common.
    let roots = |c: &[Component]| {
        c.iter()
            .take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
            .count()
    };
    if common < roots(&base).max(roots(&target)) {
        return None;
    }
    let up = base[common..]
        .iter()
        .filter(|c| matches!(c, Component::Normal(_)))
        .map(|_| "..".to_string());
    let down = target[common..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    let parts: Vec<String> = up.chain(down).collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Windows paths compare ignoring case (`C:` and `c:` are the same drive).
fn same_component(a: &Component, b: &Component) -> bool {
    if cfg!(windows) {
        a.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
    } else {
        a == b
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(path: &Path, frames: u64) -> AudioRef {
        AudioRef {
            path: path.to_path_buf(),
            relative_path: Some("take.wav".into()),
            sample_rate: 48_000,
            channels: 2,
            channel_mask: Some(3),
            encoding: "PCM".into(),
            bit_depth: 24,
            valid_bits: 24,
            frames,
            size_bytes: 1234,
            modified: Some(1_760_342_400_000),
        }
    }

    fn edited() -> EditList {
        // Fade positions like 1/3 that have no short decimal form.
        EditList::identity(3_000)
            .fade_in_range(0, 900, Curve::new(0.37))
            .unwrap()
            .delete(100, 400, 30)
            .unwrap()
            .fade_out_range(10, 2_000, Curve::new(-0.81))
            .unwrap()
            .silence(1_000, 1_100)
            .unwrap()
            .set_trim(7, 2_500)
            .set_clip_fades(
                ClipFade {
                    len: 50,
                    curve: Curve::new(0.2),
                },
                ClipFade {
                    len: 70,
                    curve: Curve::LINEAR,
                },
            )
    }

    fn sample() -> SessionFile {
        let chain = StoredChain {
            bypass_all: true,
            effects: vec![StoredEffect {
                class_id: ClassId([7; 16]),
                name: "HD Test Gain".into(),
                vendor: "HitherDither".into(),
                version: "1.0.0".into(),
                bypassed: false,
                state: StoredState {
                    component: vec![0, 1, 2, 255],
                    controller: Vec::new(),
                },
                pending_params: vec![(0, 0.1 + 0.2)],
            }],
        };
        SessionFile::new(audio(Path::new("/takes/take.wav"), 3_000), &edited(), chain)
    }

    #[test]
    fn round_trips_exactly() {
        let session = sample();
        let back = SessionFile::parse(&session.to_json(), "s").unwrap();
        assert_eq!(back, session);
        // Bit-identical fade positions, so the audio is too.
        assert_eq!(back.edit_list(), edited());
        assert_eq!(back.chain.effects[0].pending_params[0].1, 0.1 + 0.2);
    }

    #[test]
    fn ignores_unknown_fields() {
        let mut json: serde_json::Value = serde_json::from_str(&sample().to_json()).unwrap();
        json["comment"] = "added later".into();
        json["chain"]["effects"][0]["preset"] = "Warm".into();
        assert_eq!(SessionFile::parse(&json.to_string(), "s"), Ok(sample()));
    }

    #[test]
    fn rejects_other_files_and_newer_versions() {
        let invalid = Err(SessionError::Invalid("s".into()));
        assert_eq!(SessionFile::parse("hello", "s"), invalid);
        assert_eq!(SessionFile::parse("{}", "s"), invalid);
        let mut json: serde_json::Value = serde_json::from_str(&sample().to_json()).unwrap();
        json["format"] = "something-else".into();
        assert_eq!(SessionFile::parse(&json.to_string(), "s"), invalid);
        // A newer version is reported even when its layout isn't understood.
        let newer = format!(r#"{{"format":"{FORMAT}","version":{},"x":1}}"#, VERSION + 1);
        assert_eq!(
            SessionFile::parse(&newer, "s"),
            Err(SessionError::Newer("s".into()))
        );
        let mut json: serde_json::Value = serde_json::from_str(&sample().to_json()).unwrap();
        json["chain"]["effects"][0]["state"]["component"] = "not base64!".into();
        assert_eq!(SessionFile::parse(&json.to_string(), "s"), invalid);
    }

    #[test]
    fn rejects_edits_that_dont_fit() {
        let mut session = sample();
        session.audio.frames = 2_999;
        assert!(matches!(
            SessionFile::parse(&session.to_json(), "s"),
            Err(SessionError::Invalid(_))
        ));
        let mut session = sample();
        session.edits.trim = [0, 10_000];
        assert!(matches!(
            SessionFile::parse(&session.to_json(), "s"),
            Err(SessionError::Invalid(_))
        ));
    }

    #[test]
    fn recognises_session_names() {
        assert!(is_session_path(Path::new("/a/mix.hdsession.json")));
        assert!(is_session_path(Path::new("MIX.HDSESSION.JSON")));
        assert!(!is_session_path(Path::new("notes.json")));
        assert!(!is_session_path(Path::new("mix.hdsession")));
        assert!(!is_session_path(Path::new(".hdsession.json")));
    }

    #[cfg(not(windows))]
    #[test]
    fn relative_paths() {
        let r = |a: &str, b: &str| relative_path(Path::new(a), Path::new(b));
        assert_eq!(r("/takes", "/takes/take.wav").as_deref(), Some("take.wav"));
        assert_eq!(
            r("/takes", "/takes/raw/take.wav").as_deref(),
            Some("raw/take.wav")
        );
        assert_eq!(
            r("/takes/mix", "/takes/take.wav").as_deref(),
            Some("../take.wav")
        );
        assert_eq!(
            r("/a/b", "/c/take.wav").as_deref(),
            Some("../../c/take.wav")
        );
        assert_eq!(r("takes", "/takes/take.wav"), None);
    }

    #[cfg(windows)]
    #[test]
    fn relative_paths() {
        let r = |a: &str, b: &str| relative_path(Path::new(a), Path::new(b));
        assert_eq!(
            r(r"C:\takes", r"C:\takes\take.wav").as_deref(),
            Some("take.wav")
        );
        assert_eq!(
            r(r"C:\takes", r"c:\Takes\raw\take.wav").as_deref(),
            Some("raw/take.wav")
        );
        assert_eq!(
            r(r"C:\takes\mix", r"C:\takes\take.wav").as_deref(),
            Some("../take.wav")
        );
        assert_eq!(r(r"C:\takes", r"D:\takes\take.wav"), None);
    }

    #[test]
    fn resolves_absolute_then_relative() {
        let dir = std::env::temp_dir().join(format!("hd-session-{}", std::process::id()));
        let moved = dir.join("moved");
        std::fs::create_dir_all(&moved).unwrap();
        let wav = dir.join("take.wav");
        std::fs::write(&wav, b"x").unwrap();
        let mut session = sample();
        session.audio.path = wav.clone();
        session.audio.relative_path = Some("take.wav".into());
        let session_path = dir.join("mix.hdsession.json");
        assert_eq!(session.resolve_audio(&session_path), Ok(wav.clone()));

        // The absolute path is gone; the relative one is found next to the session.
        let moved_wav = moved.join("take.wav");
        std::fs::write(&moved_wav, b"x").unwrap();
        session.audio.path = dir.join("gone").join("take.wav");
        let moved_session = moved.join("mix.hdsession.json");
        assert_eq!(session.resolve_audio(&moved_session), Ok(moved_wav));

        session.audio.relative_path = None;
        assert_eq!(
            session.resolve_audio(&moved_session),
            Err(SessionError::MissingAudio("take.wav".into()))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
