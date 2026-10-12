//! VST3 effects used by HitherDither's Rust tests.
//!
//! - **HD Test Gain:** a gain parameter in dB and a fixed 64-sample delay, reported as latency.
//!   Any layout with the same 1–8 channels in and out. No editor.
//! - **HD Test Stereo:** the same processing, stereo only.
//! - **HD Test Synth:** an instrument class with no audio input, so hosts can test filtering.
//!
//! The processors share one controller class. State is the gain's normalised value as a
//! little-endian `f64`.

// The trait methods mirror the SDK's C++ interfaces, which are unsafe throughout.
#![allow(unsafe_op_in_unsafe_fn, non_snake_case)]

use std::ffi::{c_char, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{Class, ComRef, ComWrapper, uid};

/// Delay applied to every channel, and reported as latency.
pub const LATENCY: usize = 64;
/// Range of the gain parameter in dB; the normalised value maps linearly onto it.
const MIN_DB: f64 = -24.0;
const MAX_DB: f64 = 24.0;
const DEFAULT_NORM: f64 = 0.5;
const GAIN_ID: ParamID = 0;
const VENDOR: &str = "HitherDither Tests";

const GAIN_CID: TUID = uid(0x48447447, 0x41494E00, 0x8A5B5E5A, 0x00000001);
const STEREO_CID: TUID = uid(0x48447453, 0x5445524F, 0x8A5B5E5A, 0x00000002);
const SYNTH_CID: TUID = uid(0x48447453, 0x594E5448, 0x8A5B5E5A, 0x00000003);
const CONTROLLER_CID: TUID = uid(0x48444354, 0x524C0000, 0x8A5B5E5A, 0x00000004);

fn db_to_linear(norm: f64) -> f32 {
    let db = MIN_DB + norm.clamp(0.0, 1.0) * (MAX_DB - MIN_DB);
    10f64.powf(db / 20.0) as f32
}

fn copy_cstring(src: &str, dst: &mut [c_char]) {
    let n = src.len().min(dst.len() - 1);
    for (d, s) in dst.iter_mut().zip(&src.as_bytes()[..n]) {
        *d = *s as c_char;
    }
    dst[n] = 0;
}

fn copy_wstring(src: &str, dst: &mut [TChar]) {
    let mut n = 0;
    let max = dst.len() - 1;
    for (d, s) in dst.iter_mut().zip(src.encode_utf16()).take(max) {
        *d = s as TChar;
        n += 1;
    }
    dst[n] = 0;
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Gain,
    Stereo,
    Synth,
}

impl Kind {
    fn from_cid(cid: &TUID) -> Option<Kind> {
        match *cid {
            GAIN_CID => Some(Kind::Gain),
            STEREO_CID => Some(Kind::Stereo),
            SYNTH_CID => Some(Kind::Synth),
            _ => None,
        }
    }

    fn accepts(self, arr: SpeakerArrangement) -> bool {
        match self {
            Kind::Gain => (1..=8).contains(&arr.count_ones()),
            Kind::Stereo => arr == SpeakerArr::kStereo,
            Kind::Synth => false,
        }
    }
}

struct Dsp {
    /// One ring of `LATENCY` samples per channel.
    delay: Vec<[f32; LATENCY]>,
    pos: usize,
}

struct Processor {
    kind: Kind,
    gain: AtomicU64,
    arrangement: AtomicU64,
    dsp: Mutex<Dsp>,
}

impl Class for Processor {
    type Interfaces = (IComponent, IAudioProcessor);
}

impl Processor {
    fn new(kind: Kind) -> Processor {
        Processor {
            kind,
            gain: AtomicU64::new(DEFAULT_NORM.to_bits()),
            arrangement: AtomicU64::new(SpeakerArr::kStereo),
            dsp: Mutex::new(Dsp {
                delay: Vec::new(),
                pos: 0,
            }),
        }
    }

    fn channels(&self) -> i32 {
        self.arrangement.load(Ordering::Relaxed).count_ones() as i32
    }

    unsafe fn apply_param_changes(&self, changes: *mut IParameterChanges) {
        let Some(changes) = ComRef::from_raw(changes) else {
            return;
        };
        for i in 0..changes.getParameterCount() {
            let Some(queue) = ComRef::from_raw(changes.getParameterData(i)) else {
                continue;
            };
            let points = queue.getPointCount();
            if queue.getParameterId() != GAIN_ID || points == 0 {
                continue;
            }
            let (mut offset, mut value) = (0, 0.0);
            if queue.getPoint(points - 1, &mut offset, &mut value) == kResultOk {
                self.gain.store(value.to_bits(), Ordering::Relaxed);
            }
        }
    }
}

impl IPluginBaseTrait for Processor {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IComponentTrait for Processor {
    unsafe fn getControllerClassId(&self, class_id: *mut TUID) -> tresult {
        *class_id = CONTROLLER_CID;
        kResultOk
    }

    unsafe fn setIoMode(&self, _mode: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media_type: MediaType, dir: BusDirection) -> int32 {
        let audio = media_type == MediaTypes_::kAudio as MediaType;
        let input = dir == BusDirections_::kInput as BusDirection;
        match (audio, input, self.kind) {
            (true, true, Kind::Synth) => 0,
            (true, _, _) => 1,
            _ => 0,
        }
    }

    unsafe fn getBusInfo(
        &self,
        media_type: MediaType,
        dir: BusDirection,
        index: int32,
        bus: *mut BusInfo,
    ) -> tresult {
        if media_type != MediaTypes_::kAudio as MediaType
            || index >= self.getBusCount(media_type, dir)
            || index < 0
        {
            return kInvalidArgument;
        }
        let bus = &mut *bus;
        bus.mediaType = media_type;
        bus.direction = dir;
        bus.channelCount = self.channels();
        let name = if dir == BusDirections_::kInput as BusDirection {
            "Input"
        } else {
            "Output"
        };
        copy_wstring(name, &mut bus.name);
        bus.busType = BusTypes_::kMain as BusType;
        bus.flags = BusInfo_::BusFlags_::kDefaultActive as uint32;
        kResultOk
    }

    unsafe fn getRoutingInfo(&self, _in: *mut RoutingInfo, _out: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(
        &self,
        _media_type: MediaType,
        _dir: BusDirection,
        _index: int32,
        _state: TBool,
    ) -> tresult {
        kResultOk
    }

    unsafe fn setActive(&self, state: TBool) -> tresult {
        let mut dsp = self.dsp.lock().unwrap();
        dsp.delay = if state != 0 {
            vec![[0.0; LATENCY]; self.channels() as usize]
        } else {
            Vec::new()
        };
        dsp.pos = 0;
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        match read_f64(state) {
            Some(v) => {
                self.gain.store(v.to_bits(), Ordering::Relaxed);
                kResultOk
            }
            None => kResultFalse,
        }
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        write_f64(state, f64::from_bits(self.gain.load(Ordering::Relaxed)))
    }
}

impl IAudioProcessorTrait for Processor {
    unsafe fn setBusArrangements(
        &self,
        inputs: *mut SpeakerArrangement,
        num_ins: int32,
        outputs: *mut SpeakerArrangement,
        num_outs: int32,
    ) -> tresult {
        if self.kind == Kind::Synth || num_ins != 1 || num_outs != 1 {
            return kResultFalse;
        }
        let (input, output) = (*inputs, *outputs);
        if input != output || !self.kind.accepts(input) {
            return kResultFalse;
        }
        self.arrangement.store(input, Ordering::Relaxed);
        kResultTrue
    }

    unsafe fn getBusArrangement(
        &self,
        dir: BusDirection,
        index: int32,
        arr: *mut SpeakerArrangement,
    ) -> tresult {
        if index != 0 || self.getBusCount(MediaTypes_::kAudio as MediaType, dir) == 0 {
            return kInvalidArgument;
        }
        *arr = self.arrangement.load(Ordering::Relaxed);
        kResultOk
    }

    unsafe fn canProcessSampleSize(&self, size: int32) -> tresult {
        if size == SymbolicSampleSizes_::kSample32 as int32 {
            kResultTrue
        } else {
            kResultFalse
        }
    }

    unsafe fn getLatencySamples(&self) -> uint32 {
        LATENCY as uint32
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        if (*setup).symbolicSampleSize == SymbolicSampleSizes_::kSample32 as int32 {
            kResultOk
        } else {
            kResultFalse
        }
    }

    unsafe fn setProcessing(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        self.apply_param_changes(data.inputParameterChanges);
        if data.numSamples <= 0 || data.numInputs < 1 || data.numOutputs < 1 {
            return kResultOk;
        }
        let gain = db_to_linear(f64::from_bits(self.gain.load(Ordering::Relaxed)));
        let input = &*data.inputs;
        let output = &mut *data.outputs;
        output.silenceFlags = 0;
        let mut dsp = self.dsp.lock().unwrap();
        let channels = (input.numChannels.min(output.numChannels) as usize).min(dsp.delay.len());
        let n = data.numSamples as usize;
        let start = dsp.pos;
        for c in 0..channels {
            // Read each input sample before writing the output, so in-place buffers work.
            let src = *input.__field0.channelBuffers32.add(c);
            let dst = *output.__field0.channelBuffers32.add(c);
            let ring = &mut dsp.delay[c];
            let mut pos = start;
            for i in 0..n {
                let x = src.add(i).read();
                let y = ring[pos];
                ring[pos] = x;
                dst.add(i).write(y * gain);
                pos = (pos + 1) % LATENCY;
            }
        }
        dsp.pos = (start + n) % LATENCY;
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> uint32 {
        LATENCY as uint32
    }
}

struct Controller {
    gain: AtomicU64,
}

impl Class for Controller {
    type Interfaces = (IEditController,);
}

impl IPluginBaseTrait for Controller {
    unsafe fn initialize(&self, _context: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IEditControllerTrait for Controller {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        match read_f64(state) {
            Some(v) => {
                self.gain.store(v.to_bits(), Ordering::Relaxed);
                kResultOk
            }
            None => kResultFalse,
        }
    }

    unsafe fn setState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _state: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getParameterCount(&self) -> int32 {
        1
    }

    unsafe fn getParameterInfo(&self, index: int32, info: *mut ParameterInfo) -> tresult {
        if index != 0 {
            return kInvalidArgument;
        }
        let info = &mut *info;
        info.id = GAIN_ID;
        copy_wstring("Gain", &mut info.title);
        copy_wstring("Gain", &mut info.shortTitle);
        copy_wstring("dB", &mut info.units);
        info.stepCount = 0;
        info.defaultNormalizedValue = DEFAULT_NORM;
        info.unitId = 0;
        info.flags = ParameterInfo_::ParameterFlags_::kCanAutomate as int32;
        kResultOk
    }

    unsafe fn getParamStringByValue(
        &self,
        id: ParamID,
        value: ParamValue,
        string: *mut String128,
    ) -> tresult {
        if id != GAIN_ID {
            return kInvalidArgument;
        }
        let db = self.normalizedParamToPlain(id, value);
        copy_wstring(&format!("{db:.1}"), &mut *string);
        kResultOk
    }

    unsafe fn getParamValueByString(
        &self,
        id: ParamID,
        string: *mut TChar,
        value: *mut ParamValue,
    ) -> tresult {
        if id != GAIN_ID {
            return kInvalidArgument;
        }
        let mut len = 0;
        while *string.add(len) != 0 {
            len += 1;
        }
        let units: Vec<u16> = (0..len).map(|i| *string.add(i)).collect();
        match String::from_utf16_lossy(&units).trim().parse::<f64>() {
            Ok(db) => {
                *value = self.plainParamToNormalized(id, db);
                kResultOk
            }
            Err(_) => kResultFalse,
        }
    }

    unsafe fn normalizedParamToPlain(&self, _id: ParamID, value: ParamValue) -> ParamValue {
        MIN_DB + value * (MAX_DB - MIN_DB)
    }

    unsafe fn plainParamToNormalized(&self, _id: ParamID, plain: ParamValue) -> ParamValue {
        ((plain - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0)
    }

    unsafe fn getParamNormalized(&self, id: ParamID) -> ParamValue {
        if id == GAIN_ID {
            f64::from_bits(self.gain.load(Ordering::Relaxed))
        } else {
            0.0
        }
    }

    unsafe fn setParamNormalized(&self, id: ParamID, value: ParamValue) -> tresult {
        if id != GAIN_ID {
            return kInvalidArgument;
        }
        self.gain.store(value.to_bits(), Ordering::Relaxed);
        kResultOk
    }

    unsafe fn setComponentHandler(&self, _handler: *mut IComponentHandler) -> tresult {
        kResultOk
    }

    unsafe fn createView(&self, _name: FIDString) -> *mut IPlugView {
        std::ptr::null_mut()
    }
}

unsafe fn read_f64(stream: *mut IBStream) -> Option<f64> {
    let stream = ComRef::from_raw(stream)?;
    let mut bytes = [0u8; 8];
    let mut read = 0;
    let ok = stream.read(bytes.as_mut_ptr().cast(), 8, &mut read) == kResultOk;
    (ok && read == 8).then(|| f64::from_le_bytes(bytes))
}

unsafe fn write_f64(stream: *mut IBStream, value: f64) -> tresult {
    let Some(stream) = ComRef::from_raw(stream) else {
        return kInvalidArgument;
    };
    let bytes = value.to_le_bytes();
    let mut written = 0;
    match stream.write(bytes.as_ptr() as *mut c_void, 8, &mut written) {
        r if r == kResultOk && written == 8 => kResultOk,
        _ => kResultFalse,
    }
}

/// (class ID, category, name, subcategories) for each class in the factory.
const CLASSES: [(TUID, &str, &str, &str); 4] = [
    (GAIN_CID, "Audio Module Class", "HD Test Gain", "Fx|Tools"),
    (STEREO_CID, "Audio Module Class", "HD Test Stereo", "Fx|Spatial"),
    (SYNTH_CID, "Audio Module Class", "HD Test Synth", "Instrument|Synth"),
    (
        CONTROLLER_CID,
        "Component Controller Class",
        "HD Test Controller",
        "",
    ),
];

struct Factory;

impl Class for Factory {
    type Interfaces = (IPluginFactory, IPluginFactory2);
}

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        let info = &mut *info;
        copy_cstring(VENDOR, &mut info.vendor);
        copy_cstring("https://github.com/", &mut info.url);
        copy_cstring("", &mut info.email);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode as int32;
        kResultOk
    }

    unsafe fn countClasses(&self) -> int32 {
        CLASSES.len() as int32
    }

    unsafe fn getClassInfo(&self, index: int32, info: *mut PClassInfo) -> tresult {
        let Some(&(cid, category, name, _)) = CLASSES.get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid;
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
        copy_cstring(category, &mut info.category);
        copy_cstring(name, &mut info.name);
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: FIDString,
        iid: FIDString,
        obj: *mut *mut c_void,
    ) -> tresult {
        let cid = &*(cid as *const TUID);
        let instance = if *cid == CONTROLLER_CID {
            ComWrapper::new(Controller {
                gain: AtomicU64::new(DEFAULT_NORM.to_bits()),
            })
            .to_com_ptr::<FUnknown>()
        } else if let Some(kind) = Kind::from_cid(cid) {
            ComWrapper::new(Processor::new(kind)).to_com_ptr::<FUnknown>()
        } else {
            None
        };
        *obj = std::ptr::null_mut();
        match instance {
            Some(instance) => {
                let ptr = instance.as_ptr();
                ((*(*ptr).vtbl).queryInterface)(ptr, iid as *const TUID, obj)
            }
            None => kInvalidArgument,
        }
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: int32, info: *mut PClassInfo2) -> tresult {
        let Some(&(cid, category, name, sub)) = CLASSES.get(index as usize) else {
            return kInvalidArgument;
        };
        let info = &mut *info;
        info.cid = cid;
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances as int32;
        copy_cstring(category, &mut info.category);
        copy_cstring(name, &mut info.name);
        info.classFlags = 0;
        copy_cstring(sub, &mut info.subCategories);
        copy_cstring(VENDOR, &mut info.vendor);
        copy_cstring("1.0.0", &mut info.version);
        copy_cstring("VST 3.8.0", &mut info.sdkVersion);
        kResultOk
    }
}

#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
extern "system" fn InitDll() -> bool {
    true
}

#[cfg(target_os = "windows")]
#[unsafe(no_mangle)]
extern "system" fn ExitDll() -> bool {
    true
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
extern "system" fn bundleEntry(_bundle: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
extern "system" fn bundleExit() -> bool {
    true
}

#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
extern "system" fn ModuleEntry(_library: *mut c_void) -> bool {
    true
}

#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
extern "system" fn ModuleExit() -> bool {
    true
}

#[unsafe(no_mangle)]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory)
        .to_com_ptr::<IPluginFactory>()
        .map_or(std::ptr::null_mut(), |f| f.into_raw())
}
