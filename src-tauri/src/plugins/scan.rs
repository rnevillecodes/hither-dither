//! Scanning plugin bundles in a child process, so a plugin that crashes or hangs while it
//! loads can't take the app down.
//!
//! The app runs its own executable as `hither-dither --vst3-scan <bundle>` (see `main.rs`).
//! The child loads the bundle, examines each effect class and prints one result line. The
//! parent first checks that the bundle has a binary for the running OS and processor, then
//! runs up to [`Scanner::concurrency`] children at once, each with a timeout.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use vst3::Steinberg::Vst::SpeakerArrangement;

use super::ClassId;
use super::instance::{Instance, InstanceError, speaker_arrangement};
use super::module::{self, ClassInfo, Module, ModuleError};

/// The command-line flag that starts the app as a scanner.
pub const SCAN_ARG: &str = "--vst3-scan";
/// Marks the child's result line, since plugins may print to stdout too.
const RESULT_PREFIX: &str = "HD-VST3-SCAN-RESULT:";

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);
pub const DEFAULT_CONCURRENCY: usize = 4;
const POLL: Duration = Duration::from_millis(10);

pub const CRASHED: &str = "Crashed while being scanned";
pub const TIMED_OUT: &str = "Timed out while being scanned";

/// A channel layout an effect accepts on its main input and output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub channels: u16,
    /// The VST3 speaker arrangement (see [`speaker_arrangement`]).
    pub arrangement: SpeakerArrangement,
}

/// One effect class found in a bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannedClass {
    pub class_id: ClassId,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub sub_categories: String,
    /// Whether the plugin creates an editor view for this platform.
    pub has_editor: bool,
    pub layouts: Vec<Layout>,
    /// Why the class couldn't be created, when it couldn't.
    pub error: Option<String>,
}

impl ScannedClass {
    /// Whether the effect accepts a file with these channels.
    pub fn supports(&self, channels: u16, mask: Option<u32>) -> bool {
        let wanted = speaker_arrangement(channels, mask);
        self.layouts.iter().any(|l| l.arrangement == wanted)
    }
}

/// What scanning one bundle found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ScanResult {
    /// The bundle loaded; `classes` holds its effect classes, which may be none.
    Ready {
        vendor: String,
        classes: Vec<ScannedClass>,
    },
    Failed {
        reason: String,
    },
    Incompatible {
        reason: String,
    },
}

/// The reason shown for a bundle with no binary for the running OS and processor.
pub fn no_binary_reason() -> &'static str {
    if cfg!(target_os = "macos") {
        "No version for this Mac's processor"
    } else if cfg!(windows) {
        "No version for this PC's processor"
    } else {
        "No version for this computer's processor"
    }
}

// --- Child side ----------------------------------------------------------------------------

/// If the process was started with `--vst3-scan <bundle>`, scans the bundle, prints the
/// result and returns the exit code. Otherwise returns `None` and the app starts as usual.
pub fn child_main() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != SCAN_ARG {
        return None;
    }
    let Some(bundle) = args.next() else {
        return Some(2);
    };
    suppress_error_dialogs();
    let result = scan_in_process(Path::new(&bundle));
    let Ok(json) = serde_json::to_string(&result) else {
        return Some(1);
    };
    let mut out = std::io::stdout().lock();
    // The leading newline separates the result from anything the plugin printed.
    let written = writeln!(out, "\n{RESULT_PREFIX}{json}").and_then(|_| out.flush());
    Some(if written.is_ok() { 0 } else { 1 })
}

/// Keeps Windows from showing a crash dialog that would hold the child until the timeout.
fn suppress_error_dialogs() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
        };
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
}

/// Loads `bundle` in this process and examines its effect classes.
pub fn scan_in_process(bundle: &Path) -> ScanResult {
    let module = match module::load(bundle) {
        Ok(m) => m,
        Err(ModuleError::NoBinary) => {
            return ScanResult::Incompatible {
                reason: no_binary_reason().into(),
            };
        }
        Err(e) => {
            return ScanResult::Failed {
                reason: sentence(&e.to_string()),
            };
        }
    };
    let classes = module
        .classes()
        .into_iter()
        .filter(is_effect_candidate)
        .filter_map(|c| scan_class(&module, c))
        .collect();
    ScanResult::Ready {
        vendor: module.vendor(),
        classes,
    }
}

/// Audio classes that may be effects: those marked `Fx` and not `Instrument`, and those with
/// no subcategories, which are kept only if they turn out to have audio in and out.
fn is_effect_candidate(class: &ClassInfo) -> bool {
    let has = |name| class.sub_category_list().any(|s| s == name);
    class.category == "Audio Module Class"
        && (class.sub_categories.is_empty() || (has("Fx") && !has("Instrument")))
}

fn scan_class(module: &Arc<Module>, info: ClassInfo) -> Option<ScannedClass> {
    let mut class = ScannedClass {
        class_id: info.cid,
        name: info.name,
        vendor: info.vendor,
        version: info.version,
        sub_categories: info.sub_categories,
        has_editor: false,
        layouts: Vec::new(),
        error: None,
    };
    let instance = match Instance::create(module.clone(), info.cid) {
        Ok(i) => i,
        Err(InstanceError::NotAnEffect) => return None,
        Err(e) => {
            class.error = Some(sentence(&e.to_string()));
            return Some(class);
        }
    };
    for (channels, mask) in probes() {
        let arrangement = speaker_arrangement(channels, mask);
        if class.layouts.iter().any(|l| l.arrangement == arrangement) {
            continue;
        }
        match instance.try_arrangement(arrangement) {
            Ok(()) => class.layouts.push(Layout {
                channels,
                arrangement,
            }),
            Err(InstanceError::NotAnEffect) => return None,
            Err(_) => {}
        }
    }
    class.layouts.sort_by_key(|l| (l.channels, l.arrangement));
    class.has_editor = instance.has_editor();
    Some(class)
}

/// The layouts HitherDither can open: mono, stereo, the WAV masks for quad, 5.0, 5.1 and 7.1,
/// and n discrete channels for 1 to 8.
fn probes() -> Vec<(u16, Option<u32>)> {
    let named = [
        (1, None),
        (2, Some(0x3)),
        (4, Some(0x33)),
        (5, Some(0x37)),
        (6, Some(0x3F)),
        (8, Some(0x63F)),
    ];
    named
        .into_iter()
        .chain((1..=8).map(|n| (n, None)))
        .collect()
}

/// Capitalises an error message for display.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

// --- Parent side ---------------------------------------------------------------------------

/// Runs scans in child processes.
pub struct Scanner {
    exe: PathBuf,
    pub timeout: Duration,
    pub concurrency: usize,
    launches: AtomicUsize,
}

impl Scanner {
    /// A scanner that runs `exe --vst3-scan <bundle>`; `exe` is normally the app itself.
    pub fn new(exe: PathBuf) -> Scanner {
        Scanner {
            exe,
            timeout: DEFAULT_TIMEOUT,
            concurrency: DEFAULT_CONCURRENCY,
            launches: AtomicUsize::new(0),
        }
    }

    /// How many child processes this scanner has started.
    pub fn launches(&self) -> usize {
        self.launches.load(Ordering::Relaxed)
    }

    /// Scans one bundle, without a child process when it has no binary for this platform.
    pub fn scan(&self, bundle: &Path) -> ScanResult {
        if !has_running_binary(bundle) {
            return ScanResult::Incompatible {
                reason: no_binary_reason().into(),
            };
        }
        self.launch(bundle)
    }

    /// Scans `bundles`, up to `concurrency` at once, calling `on_result` on this thread as
    /// each finishes.
    pub fn scan_all(&self, bundles: &[PathBuf], mut on_result: impl FnMut(&Path, ScanResult)) {
        let next = AtomicUsize::new(0);
        let (tx, rx) = mpsc::channel();
        thread::scope(|s| {
            for _ in 0..self.concurrency.clamp(1, bundles.len().max(1)) {
                let (tx, next) = (tx.clone(), &next);
                s.spawn(move || {
                    while let Some(bundle) = bundles.get(next.fetch_add(1, Ordering::Relaxed)) {
                        if tx.send((bundle, self.scan(bundle))).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(tx);
            for (bundle, result) in rx {
                on_result(bundle, result);
            }
        });
    }

    fn launch(&self, bundle: &Path) -> ScanResult {
        self.launches.fetch_add(1, Ordering::Relaxed);
        let mut cmd = Command::new(&self.exe);
        cmd.arg(SCAN_ARG)
            .arg(bundle)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return ScanResult::Failed {
                    reason: format!("The scanner couldn't start: {e}"),
                };
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return ScanResult::Failed {
                reason: CRASHED.into(),
            };
        };
        // Read on another thread so a chatty plugin can't fill the pipe and stall.
        let reader = thread::spawn(move || {
            let mut out = Vec::new();
            let _ = stdout.read_to_end(&mut out);
            out
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if Instant::now() < deadline => thread::sleep(POLL),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        match status {
            // The reader is left to finish on its own: something the plugin started may still
            // hold the pipe open.
            None => ScanResult::Failed {
                reason: TIMED_OUT.into(),
            },
            Some(s) if !s.success() => ScanResult::Failed {
                reason: CRASHED.into(),
            },
            Some(_) => reader
                .join()
                .ok()
                .and_then(|out| parse_output(&out))
                .unwrap_or_else(|| ScanResult::Failed {
                    reason: CRASHED.into(),
                }),
        }
    }
}

fn parse_output(out: &[u8]) -> Option<ScanResult> {
    let text = String::from_utf8_lossy(out);
    let line = text
        .lines()
        .rev()
        .find_map(|l| l.strip_prefix(RESULT_PREFIX))?;
    serde_json::from_str(line).ok()
}

// --- Architecture check --------------------------------------------------------------------

/// Whether `bundle` has a binary the running process can load. A binary whose format isn't
/// recognised is given the benefit of the doubt; the scan itself then decides.
pub fn has_running_binary(bundle: &Path) -> bool {
    let Some(binary) = module::binary_path(bundle) else {
        return false;
    };
    let mut header = Vec::new();
    let read =
        std::fs::File::open(&binary).and_then(|f| f.take(HEADER_BYTES).read_to_end(&mut header));
    read.is_err() || matches_running_arch(&header) != Some(false)
}

const HEADER_BYTES: u64 = 64 * 1024;

fn matches_running_arch(header: &[u8]) -> Option<bool> {
    if cfg!(windows) {
        pe_machine(header).map(|m| m == PE_MACHINE)
    } else if cfg!(target_os = "macos") {
        macho_cpu_types(header).map(|cpus| cpus.contains(&MACHO_CPU))
    } else {
        elf_machine(header).map(|m| m == ELF_MACHINE)
    }
}

const PE_I386: u16 = 0x14C;
const PE_AMD64: u16 = 0x8664;
const PE_ARM64: u16 = 0xAA64;
const PE_MACHINE: u16 = if cfg!(target_arch = "aarch64") {
    PE_ARM64
} else if cfg!(target_arch = "x86") {
    PE_I386
} else {
    PE_AMD64
};

const ELF_386: u16 = 3;
const ELF_X86_64: u16 = 62;
const ELF_AARCH64: u16 = 183;
const ELF_MACHINE: u16 = if cfg!(target_arch = "aarch64") {
    ELF_AARCH64
} else if cfg!(target_arch = "x86") {
    ELF_386
} else {
    ELF_X86_64
};

const MACHO_X86_64: u32 = 0x0100_0007;
const MACHO_ARM64: u32 = 0x0100_000C;
const MACHO_CPU: u32 = if cfg!(target_arch = "aarch64") {
    MACHO_ARM64
} else {
    MACHO_X86_64
};

fn u16_at(bytes: &[u8], at: usize, big_endian: bool) -> Option<u16> {
    let b: [u8; 2] = bytes.get(at..at + 2)?.try_into().ok()?;
    Some(if big_endian {
        u16::from_be_bytes(b)
    } else {
        u16::from_le_bytes(b)
    })
}

fn u32_at(bytes: &[u8], at: usize, big_endian: bool) -> Option<u32> {
    let b: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(if big_endian {
        u32::from_be_bytes(b)
    } else {
        u32::from_le_bytes(b)
    })
}

/// The machine type of a PE (Windows DLL) image.
fn pe_machine(header: &[u8]) -> Option<u16> {
    if !header.starts_with(b"MZ") {
        return None;
    }
    let pe = u32_at(header, 0x3C, false)? as usize;
    if header.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    u16_at(header, pe + 4, false)
}

/// The machine type of an ELF shared object.
fn elf_machine(header: &[u8]) -> Option<u16> {
    if !header.starts_with(b"\x7FELF") {
        return None;
    }
    let big_endian = *header.get(5)? == 2;
    u16_at(header, 18, big_endian)
}

/// The CPU types in a Mach-O binary, thin or universal.
fn macho_cpu_types(header: &[u8]) -> Option<Vec<u32>> {
    let magic = u32_at(header, 0, true)?;
    match magic {
        // Thin, 32- or 64-bit, little-endian.
        0xCEFA_EDFE | 0xCFFA_EDFE => Some(vec![u32_at(header, 4, false)?]),
        // Universal: big-endian headers of 20 bytes (fat_arch) or 32 (fat_arch_64).
        0xCAFE_BABE | 0xCAFE_BABF => {
            let size = if magic == 0xCAFE_BABE { 20 } else { 32 };
            let count = u32_at(header, 4, true)? as usize;
            (0..count.min(64))
                .map(|i| u32_at(header, 8 + i * size, true))
                .collect()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe(machine: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x100];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    fn elf(machine: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 64];
        bytes[..4].copy_from_slice(b"\x7FELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    fn universal(cpus: &[u32]) -> Vec<u8> {
        let mut bytes = 0xCAFE_BABEu32.to_be_bytes().to_vec();
        bytes.extend((cpus.len() as u32).to_be_bytes());
        for &cpu in cpus {
            let mut arch = [0u8; 20];
            arch[..4].copy_from_slice(&cpu.to_be_bytes());
            bytes.extend(arch);
        }
        bytes
    }

    #[test]
    fn reads_binary_architectures() {
        assert_eq!(pe_machine(&pe(PE_AMD64)), Some(PE_AMD64));
        assert_eq!(pe_machine(&pe(PE_ARM64)), Some(PE_ARM64));
        assert_eq!(pe_machine(b"MZ"), None);
        assert_eq!(elf_machine(&elf(ELF_AARCH64)), Some(ELF_AARCH64));
        assert_eq!(elf_machine(&pe(PE_AMD64)), None);

        let mut thin = 0xCFFA_EDFEu32.to_be_bytes().to_vec();
        thin.extend(MACHO_ARM64.to_le_bytes());
        assert_eq!(macho_cpu_types(&thin), Some(vec![MACHO_ARM64]));
        // An Intel-only plugin has no arm64 slice.
        let intel = universal(&[MACHO_X86_64]);
        assert_eq!(macho_cpu_types(&intel), Some(vec![MACHO_X86_64]));
        let both = universal(&[MACHO_X86_64, MACHO_ARM64]);
        assert_eq!(
            macho_cpu_types(&both),
            Some(vec![MACHO_X86_64, MACHO_ARM64])
        );
        assert_eq!(macho_cpu_types(b"\0\0\0\0"), None);
    }

    #[test]
    fn the_running_binary_matches_itself() {
        let exe = std::env::current_exe().unwrap();
        let bytes = std::fs::read(exe).unwrap();
        assert_eq!(matches_running_arch(&bytes), Some(true));
    }

    #[test]
    fn result_line_is_found_after_plugin_output() {
        let result = ScanResult::Failed { reason: "x".into() };
        let json = serde_json::to_string(&result).unwrap();
        let out = format!("plugin says hi\n{RESULT_PREFIX}{json}\n");
        assert_eq!(parse_output(out.as_bytes()), Some(result));
        assert_eq!(parse_output(b"garbage"), None);
    }

    #[test]
    fn layouts_match_files() {
        let class = ScannedClass {
            class_id: ClassId([0; 16]),
            name: String::new(),
            vendor: String::new(),
            version: String::new(),
            sub_categories: String::new(),
            has_editor: false,
            layouts: vec![Layout {
                channels: 2,
                arrangement: speaker_arrangement(2, None),
            }],
            error: None,
        };
        assert!(class.supports(2, Some(0x3)));
        assert!(class.supports(2, None));
        assert!(!class.supports(6, Some(0x3F)));
    }
}
