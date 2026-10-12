//! Loading `.vst3` modules and reading their factories.
//!
//! - **Windows:** `Contents/<arch>-win/<name>.vst3` inside a bundle folder, or a single-file DLL;
//!   `InitDll`/`ExitDll`.
//! - **macOS:** a `CFBundle`; `bundleEntry`/`bundleExit`.
//! - **Linux:** `Contents/<arch>-linux/<name>.so`; `ModuleEntry`/`ModuleExit`.
//!
//! Loaded modules are cached per bundle path for the life of the process: many plugins
//! misbehave when unloaded and loaded again.

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use vst3::Steinberg::kResultOk;
use vst3::Steinberg::{IPluginFactory, IPluginFactory2, PClassInfo, PClassInfo2, PFactoryInfo};
use vst3::Steinberg::{IPluginFactory2Trait, IPluginFactoryTrait};
use vst3::{ComPtr, Interface};

use super::{ClassId, from_cstr};

#[derive(Debug, Clone, thiserror::Error)]
pub enum ModuleError {
    #[error("no binary for this platform")]
    NoBinary,
    #[error("could not load the plugin: {0}")]
    Load(String),
    #[error("the plugin failed to initialise its module")]
    Entry,
    #[error("the plugin has no VST3 factory")]
    NoFactory,
}

/// One class in a plugin factory.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassInfo {
    pub cid: ClassId,
    /// For example "Audio Module Class".
    pub category: String,
    pub name: String,
    /// The `|`-separated subcategories, for example "Fx|EQ". Empty without `IPluginFactory2`.
    pub sub_categories: String,
    /// The class vendor, or the factory's when the class has none.
    pub vendor: String,
    pub version: String,
}

impl ClassInfo {
    pub fn sub_category_list(&self) -> impl Iterator<Item = &str> {
        self.sub_categories.split('|').filter(|s| !s.is_empty())
    }
}

/// A loaded plugin module and its factory.
pub struct Module {
    path: PathBuf,
    factory: ManuallyDrop<ComPtr<IPluginFactory>>,
    library: platform::Library,
}

// SAFETY: the factory is a COM object the VST3 spec allows to be called from the UI thread;
// callers marshal there. The library handle is only used on drop.
unsafe impl Send for Module {}
unsafe impl Sync for Module {}

impl Module {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn factory(&self) -> &ComPtr<IPluginFactory> {
        &self.factory
    }

    /// The factory's vendor.
    pub fn vendor(&self) -> String {
        let mut info: PFactoryInfo = unsafe { std::mem::zeroed() };
        if unsafe { self.factory.getFactoryInfo(&mut info) } == kResultOk {
            from_cstr(&info.vendor)
        } else {
            String::new()
        }
    }

    /// Every class in the factory, with `IPluginFactory2` details when the plugin offers them.
    pub fn classes(&self) -> Vec<ClassInfo> {
        let factory_vendor = self.vendor();
        let factory2 = self.factory.cast::<IPluginFactory2>();
        let count = unsafe { self.factory.countClasses() }.max(0);
        let mut classes = Vec::with_capacity(count as usize);
        for i in 0..count {
            let mut info2: PClassInfo2 = unsafe { std::mem::zeroed() };
            let class = match &factory2 {
                Some(f) if unsafe { f.getClassInfo2(i, &mut info2) } == kResultOk => ClassInfo {
                    cid: ClassId::from_tuid(&info2.cid),
                    category: from_cstr(&info2.category),
                    name: from_cstr(&info2.name),
                    sub_categories: from_cstr(&info2.subCategories),
                    vendor: from_cstr(&info2.vendor),
                    version: from_cstr(&info2.version),
                },
                _ => {
                    let mut info: PClassInfo = unsafe { std::mem::zeroed() };
                    if unsafe { self.factory.getClassInfo(i, &mut info) } != kResultOk {
                        continue;
                    }
                    ClassInfo {
                        cid: ClassId::from_tuid(&info.cid),
                        category: from_cstr(&info.category),
                        name: from_cstr(&info.name),
                        sub_categories: String::new(),
                        vendor: String::new(),
                        version: String::new(),
                    }
                }
            };
            classes.push(ClassInfo {
                vendor: if class.vendor.is_empty() {
                    factory_vendor.clone()
                } else {
                    class.vendor
                },
                ..class
            });
        }
        classes
    }

    /// Creates an instance of `cid` and queries it for `I`. Call on the UI thread.
    pub fn create<I: Interface>(&self, cid: ClassId) -> Option<ComPtr<I>> {
        let tuid = cid.to_tuid();
        let mut obj: *mut c_void = std::ptr::null_mut();
        let result = unsafe {
            self.factory
                .createInstance(tuid.as_ptr(), I::IID.as_ptr().cast(), &mut obj)
        };
        if result != kResultOk {
            return None;
        }
        unsafe { ComPtr::from_raw(obj.cast::<I>()) }
    }
}

impl Drop for Module {
    fn drop(&mut self) {
        // The factory is released before the module's exit function runs.
        unsafe { ManuallyDrop::drop(&mut self.factory) };
        self.library.exit();
    }
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Arc<Module>>> {
    static MODULES: OnceLock<Mutex<HashMap<PathBuf, Arc<Module>>>> = OnceLock::new();
    MODULES.get_or_init(Default::default)
}

/// Loads the module at `bundle`, or returns the one already loaded in this process.
pub fn load(bundle: &Path) -> Result<Arc<Module>, ModuleError> {
    let mut modules = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = modules.get(bundle) {
        return Ok(m.clone());
    }
    let module = Arc::new(load_uncached(bundle)?);
    modules.insert(bundle.to_path_buf(), module.clone());
    Ok(module)
}

fn load_uncached(bundle: &Path) -> Result<Module, ModuleError> {
    let library = platform::Library::open(bundle)?;
    let factory = library.factory().ok_or_else(|| {
        library.exit();
        ModuleError::NoFactory
    })?;
    Ok(Module {
        path: bundle.to_path_buf(),
        factory: ManuallyDrop::new(factory),
        library,
    })
}

type GetFactory = unsafe extern "system" fn() -> *mut IPluginFactory;

/// The bundle subfolder names that can hold a binary for the running OS and architecture,
/// relative to `Contents/`, in order of preference.
pub fn binary_dirs() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["MacOS"]
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        &["x86_64-win"]
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        &["arm64-win", "arm64x-win", "arm64ec-win"]
    } else if cfg!(all(windows, target_arch = "x86")) {
        &["x86-win"]
    } else if cfg!(target_arch = "aarch64") {
        &["aarch64-linux"]
    } else if cfg!(target_arch = "x86") {
        &["i386-linux"]
    } else {
        &["x86_64-linux"]
    }
}

/// The plugin binary inside `bundle` for this OS and architecture, if there is one. On macOS
/// this is the bundle's executable, which may still lack a slice for the running processor.
pub fn binary_path(bundle: &Path) -> Option<PathBuf> {
    if bundle.is_file() {
        // A single-file plugin, as older Windows plugins are distributed.
        return cfg!(windows).then(|| bundle.to_path_buf());
    }
    let contents = bundle.join("Contents");
    let stem = bundle.file_stem()?.to_string_lossy().into_owned();
    if cfg!(target_os = "macos") {
        let macos = contents.join("MacOS");
        let named = macos.join(&stem);
        if named.is_file() {
            return Some(named);
        }
        // The executable name comes from Info.plist; fall back to the only file there.
        let mut files = std::fs::read_dir(&macos).ok()?.flatten().map(|e| e.path());
        let first = files.next().filter(|p| p.is_file());
        return first.filter(|_| files.next().is_none());
    }
    let ext = if cfg!(windows) { "vst3" } else { "so" };
    binary_dirs()
        .iter()
        .map(|dir| contents.join(dir).join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

#[cfg(any(windows, target_os = "linux"))]
mod platform {
    use super::*;

    pub struct Library {
        lib: libloading::Library,
    }

    #[cfg(windows)]
    const ENTRY: &[u8] = b"InitDll\0";
    #[cfg(windows)]
    const EXIT: &[u8] = b"ExitDll\0";
    #[cfg(target_os = "linux")]
    const ENTRY: &[u8] = b"ModuleEntry\0";
    #[cfg(target_os = "linux")]
    const EXIT: &[u8] = b"ModuleExit\0";

    impl Library {
        pub fn open(bundle: &Path) -> Result<Library, ModuleError> {
            let path = binary_path(bundle).ok_or(ModuleError::NoBinary)?;
            let (lib, handle) =
                open_library(&path).map_err(|e| ModuleError::Load(e.to_string()))?;
            // The entry function is optional on Windows and expected on Linux; call it when
            // present.
            #[cfg(windows)]
            let ok = {
                let _ = handle;
                unsafe {
                    lib.get::<unsafe extern "system" fn() -> bool>(ENTRY)
                        .map(|f| f())
                }
            };
            #[cfg(target_os = "linux")]
            let ok = unsafe {
                lib.get::<unsafe extern "system" fn(*mut c_void) -> bool>(ENTRY)
                    .map(|f| f(handle))
            };
            if matches!(ok, Ok(false)) {
                return Err(ModuleError::Entry);
            }
            Ok(Library { lib })
        }

        pub fn factory(&self) -> Option<ComPtr<IPluginFactory>> {
            unsafe {
                let get = self.lib.get::<GetFactory>(b"GetPluginFactory\0").ok()?;
                ComPtr::from_raw(get())
            }
        }

        pub fn exit(&self) {
            unsafe {
                if let Ok(exit) = self.lib.get::<unsafe extern "system" fn() -> bool>(EXIT) {
                    exit();
                }
            }
        }
    }

    type Opened = (libloading::Library, *mut c_void);

    #[cfg(windows)]
    fn open_library(path: &Path) -> Result<Opened, libloading::Error> {
        let lib = unsafe { libloading::Library::new(path) }?;
        Ok((lib, std::ptr::null_mut()))
    }

    /// Also returns the `dlopen` handle, which `ModuleEntry` receives.
    #[cfg(target_os = "linux")]
    fn open_library(path: &Path) -> Result<Opened, libloading::Error> {
        use libloading::os::unix::{Library, RTLD_LOCAL, RTLD_NOW};
        let handle = unsafe { Library::open(Some(path), RTLD_NOW | RTLD_LOCAL) }?.into_raw();
        let lib = unsafe { Library::from_raw(handle) };
        Ok((lib.into(), handle))
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use core_foundation::base::TCFType;
    use core_foundation::bundle::{CFBundle, CFBundleLoadExecutable};
    use core_foundation::string::CFString;
    use core_foundation::url::CFURL;

    pub struct Library {
        bundle: CFBundle,
    }

    impl Library {
        pub fn open(path: &Path) -> Result<Library, ModuleError> {
            if binary_path(path).is_none() {
                return Err(ModuleError::NoBinary);
            }
            let url = CFURL::from_path(path, true)
                .ok_or_else(|| ModuleError::Load("invalid path".into()))?;
            let bundle =
                CFBundle::new(url).ok_or_else(|| ModuleError::Load("not a valid bundle".into()))?;
            if unsafe { CFBundleLoadExecutable(bundle.as_concrete_TypeRef()) } == 0 {
                return Err(ModuleError::Load(
                    "the executable could not be loaded".into(),
                ));
            }
            let lib = Library { bundle };
            let entry = lib
                .symbol("bundleEntry")
                .or_else(|| lib.symbol("BundleEntry"));
            if let Some(entry) = entry {
                let entry: unsafe extern "system" fn(*mut c_void) -> bool =
                    unsafe { std::mem::transmute(entry) };
                if !unsafe { entry(lib.bundle.as_concrete_TypeRef() as *mut c_void) } {
                    return Err(ModuleError::Entry);
                }
            }
            Ok(lib)
        }

        fn symbol(&self, name: &str) -> Option<*const c_void> {
            let ptr = self.bundle.function_pointer_for_name(CFString::new(name));
            (!ptr.is_null()).then_some(ptr)
        }

        pub fn factory(&self) -> Option<ComPtr<IPluginFactory>> {
            let get: GetFactory = unsafe { std::mem::transmute(self.symbol("GetPluginFactory")?) };
            unsafe { ComPtr::from_raw(get()) }
        }

        pub fn exit(&self) {
            if let Some(exit) = self
                .symbol("bundleExit")
                .or_else(|| self.symbol("BundleExit"))
            {
                let exit: unsafe extern "system" fn() -> bool =
                    unsafe { std::mem::transmute(exit) };
                unsafe { exit() };
            }
        }
    }
}
