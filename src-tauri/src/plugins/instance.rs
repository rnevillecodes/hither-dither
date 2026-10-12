//! One effect instance: its component, processor and controller.
//!
//! Creation, configuration, state, and dropping run on the UI thread. [`Instance::process`]
//! runs on whichever thread holds the processing lock, never on two at once.

use std::sync::{Arc, Mutex, MutexGuard};

use vst3::Steinberg::Vst::ParameterInfo_::ParameterFlags_::kIsBypass;
use vst3::Steinberg::Vst::ProcessContext_::StatesAndFlags_::{
    kPlaying, kProjectTimeMusicValid, kTempoValid, kTimeSigValid,
};
use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{ComPtr, ComWrapper};

use super::ClassId;
use super::host::{
    ComponentHandler, EventList, HostApplication, MemoryStream, ParameterChanges, PendingParams,
};
use super::module::Module;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InstanceError {
    #[error("the plugin could not be created")]
    Create,
    #[error("the plugin failed to initialise")]
    Initialize,
    #[error("the plugin isn't an audio effect")]
    NotAnEffect,
    #[error("the plugin doesn't support this channel layout")]
    Layout,
    #[error("the plugin failed to set up processing")]
    Setup,
}

/// The VST3 speaker arrangement for a WAV file's channels.
///
/// WAV channel-mask bits and VST3 speaker bits agree for the first 18 speakers (L, R, C, LFE,
/// back L/R as Ls/Rs, …), so a mask with one bit per channel maps directly, keeping channel
/// order. 5.x with side instead of back surrounds is mapped to Ls/Rs, as VST3's 5.x layouts
/// expect. Without a usable mask, one channel is mono, two are stereo, and other counts use
/// the lowest speakers in order ("n discrete channels").
pub fn speaker_arrangement(channels: u16, mask: Option<u32>) -> SpeakerArrangement {
    const BACK: u32 = 0x30;
    const SIDE: u32 = 0x600;
    match (channels, mask) {
        (1, _) => SpeakerArr::kMono,
        (n, Some(m)) if m.count_ones() == n as u32 && m < 1 << 18 => {
            let m = if m & SIDE == SIDE && m & BACK == 0 {
                (m & !SIDE) | BACK
            } else {
                m
            };
            m as SpeakerArrangement
        }
        (2, _) => SpeakerArr::kStereo,
        (n, _) => (1u64 << n.min(64)).wrapping_sub(1),
    }
}

/// The audio format an instance processes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    pub channels: u16,
    pub channel_mask: Option<u32>,
    pub sample_rate: f64,
    pub max_block: usize,
    pub offline: bool,
}

/// Where the audio being processed is, for the plugin's `ProcessContext`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Transport {
    pub playing: bool,
    /// The project time of the block's first frame, in frames.
    pub position: i64,
}

/// Plugin state as saved by `getState`: the component's and the controller's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub component: Vec<u8>,
    pub controller: Vec<u8>,
}

/// Processing-thread state, behind the processing lock.
struct Processing {
    config: Option<Config>,
    active: bool,
    processing: bool,
    /// Channel counts of the non-main buses, which get silence in and scratch out.
    aux_in: Vec<i32>,
    aux_out: Vec<i32>,
    silence: Vec<f32>,
    scratch: Vec<f32>,
    input_changes: ComWrapper<ParameterChanges>,
    output_changes: ComWrapper<ParameterChanges>,
    events: ComWrapper<EventList>,
}

pub struct Instance {
    cid: ClassId,
    // Fields drop in declaration order after `Drop::drop`: the plugin's objects go before the
    // host objects they may still reference, and the module goes last.
    connection: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
    controller: Option<ComPtr<IEditController>>,
    separate_controller: bool,
    processor: ComPtr<IAudioProcessor>,
    component: ComPtr<IComponent>,
    proc: Mutex<Processing>,
    handler: ComWrapper<ComponentHandler>,
    _host: ComWrapper<HostApplication>,
    pending: Arc<PendingParams>,
    bypass_param: Option<ParamID>,
    _module: Arc<Module>,
}

// SAFETY: plugin calls are confined by the threading rules in the module docs: UI-thread
// methods are only called from the UI thread, and processing goes through the `proc` lock.
unsafe impl Send for Instance {}
unsafe impl Sync for Instance {}

const AUDIO: MediaType = MediaTypes_::kAudio as MediaType;
const INPUT: BusDirection = BusDirections_::kInput as BusDirection;
const OUTPUT: BusDirection = BusDirections_::kOutput as BusDirection;

/// The view platform type native editors are attached with on this OS.
#[cfg(windows)]
const PLATFORM_TYPE: FIDString = kPlatformTypeHWND;
#[cfg(target_os = "macos")]
const PLATFORM_TYPE: FIDString = kPlatformTypeNSView;
#[cfg(not(any(windows, target_os = "macos")))]
const PLATFORM_TYPE: FIDString = kPlatformTypeX11EmbedWindowID;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn ok(result: tresult) -> bool {
    result == kResultOk
}

impl Instance {
    /// Creates and initialises `cid`, connects its controller and syncs the controller to the
    /// component's state. UI thread.
    pub fn create(module: Arc<Module>, cid: ClassId) -> Result<Instance, InstanceError> {
        let component = module
            .create::<IComponent>(cid)
            .ok_or(InstanceError::Create)?;
        let host = ComWrapper::new(HostApplication);
        let context = host
            .as_com_ref::<FUnknown>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        if !ok(unsafe { component.initialize(context) }) {
            return Err(InstanceError::Initialize);
        }
        let Some(processor) = component.cast::<IAudioProcessor>() else {
            unsafe { component.terminate() };
            return Err(InstanceError::NotAnEffect);
        };

        let (controller, separate_controller) = match component.cast::<IEditController>() {
            Some(c) => (Some(c), false),
            None => {
                let mut tuid: TUID = [0; 16];
                let created = ok(unsafe { component.getControllerClassId(&mut tuid) })
                    .then(|| module.create::<IEditController>(ClassId::from_tuid(&tuid)))
                    .flatten();
                let initialised = created.filter(|c| ok(unsafe { c.initialize(context) }));
                (initialised, true)
            }
        };

        let pending = Arc::new(PendingParams::default());
        let handler = ComWrapper::new(ComponentHandler::new(pending.clone()));
        let mut connection = None;
        if let Some(c) = &controller {
            if let Some(h) = handler.as_com_ref::<IComponentHandler>() {
                unsafe { c.setComponentHandler(h.as_ptr()) };
            }
            if separate_controller {
                let pair = component
                    .cast::<IConnectionPoint>()
                    .zip(c.cast::<IConnectionPoint>());
                if let Some((a, b)) = pair {
                    unsafe {
                        a.connect(b.as_ptr());
                        b.connect(a.as_ptr());
                    }
                    connection = Some((a, b));
                }
                let stream = ComWrapper::new(MemoryStream::default());
                if let Some(s) = stream.as_com_ref::<IBStream>()
                    && ok(unsafe { component.getState(s.as_ptr()) })
                {
                    stream.rewind();
                    unsafe { c.setComponentState(s.as_ptr()) };
                }
            }
        }

        let bypass_param = controller.as_ref().and_then(find_bypass_param);
        let proc = Processing {
            config: None,
            active: false,
            processing: false,
            aux_in: Vec::new(),
            aux_out: Vec::new(),
            silence: Vec::new(),
            scratch: Vec::new(),
            input_changes: ComWrapper::new(ParameterChanges::default()),
            output_changes: ComWrapper::new(ParameterChanges::default()),
            events: ComWrapper::new(EventList),
        };
        Ok(Instance {
            cid,
            connection,
            controller,
            separate_controller,
            processor,
            component,
            proc: Mutex::new(proc),
            handler,
            _host: host,
            pending,
            bypass_param,
            _module: module,
        })
    }

    pub fn class_id(&self) -> ClassId {
        self.cid
    }

    pub fn controller(&self) -> Option<&ComPtr<IEditController>> {
        self.controller.as_ref()
    }

    pub fn handler(&self) -> &ComponentHandler {
        &self.handler
    }

    /// Parameter values waiting for the next `process` call.
    pub fn pending(&self) -> &PendingParams {
        &self.pending
    }

    /// The parameter flagged `kIsBypass`, if the plugin has one.
    pub fn bypass_param(&self) -> Option<ParamID> {
        self.bypass_param
    }

    /// The current configuration, if configured.
    pub fn config(&self) -> Option<Config> {
        lock(&self.proc).config
    }

    /// Sets a parameter on the controller and queues it for the processor. UI thread.
    pub fn set_param(&self, id: ParamID, value: f64) {
        if let Some(c) = &self.controller {
            unsafe { c.setParamNormalized(id, value) };
        }
        self.pending.set(id, value);
    }

    /// Negotiates the bus layout, sets up processing and activates the plugin, deactivating
    /// it first if it was active. Returns the latency in frames. UI thread; waits for any
    /// processing in progress.
    pub fn configure(&self, config: Config) -> Result<u32, InstanceError> {
        let mut proc = lock(&self.proc);
        self.deactivate(&mut proc);
        proc.config = None;

        let arrangement = speaker_arrangement(config.channels, config.channel_mask);
        let (ins, outs) = self.negotiate(arrangement)?;
        proc.aux_in = (1..ins)
            .map(|i| self.arrangement(INPUT, i).count_ones() as i32)
            .collect();
        proc.aux_out = (1..outs)
            .map(|i| self.arrangement(OUTPUT, i).count_ones() as i32)
            .collect();

        unsafe {
            for (dir, count) in [(INPUT, ins), (OUTPUT, outs)] {
                for i in 0..count {
                    self.component.activateBus(AUDIO, dir, i, (i == 0) as TBool);
                }
            }
            let event = MediaTypes_::kEvent as MediaType;
            for dir in [INPUT, OUTPUT] {
                for i in 0..self.component.getBusCount(event, dir) {
                    self.component.activateBus(event, dir, i, 0);
                }
            }
        }

        let sample32 = SymbolicSampleSizes_::kSample32 as int32;
        if unsafe { self.processor.canProcessSampleSize(sample32) } != kResultTrue {
            return Err(InstanceError::Setup);
        }
        let mode = if config.offline {
            ProcessModes_::kOffline
        } else {
            ProcessModes_::kRealtime
        };
        let mut setup = ProcessSetup {
            processMode: mode as int32,
            symbolicSampleSize: sample32,
            maxSamplesPerBlock: config.max_block as int32,
            sampleRate: config.sample_rate,
        };
        if !ok(unsafe { self.processor.setupProcessing(&mut setup) }) {
            return Err(InstanceError::Setup);
        }
        if !ok(unsafe { self.component.setActive(1) }) {
            return Err(InstanceError::Setup);
        }
        proc.active = true;
        proc.silence = vec![0.0; config.max_block];
        proc.scratch = vec![0.0; config.max_block];
        proc.config = Some(config);
        Ok(self.latency())
    }

    /// Tries `arrangement` on the main input and output buses: fails with `Layout` when the
    /// plugin refuses it, or `NotAnEffect` when it has no audio input or output. Leaves the
    /// instance deactivated and unconfigured. UI thread.
    pub fn try_arrangement(&self, arrangement: SpeakerArrangement) -> Result<(), InstanceError> {
        let mut proc = lock(&self.proc);
        self.deactivate(&mut proc);
        proc.config = None;
        self.negotiate(arrangement).map(|_| ())
    }

    /// Whether the controller creates an editor view that can be attached on this platform.
    /// UI thread.
    pub fn has_editor(&self) -> bool {
        let Some(c) = &self.controller else {
            return false;
        };
        let view = unsafe { c.createView(ViewType::kEditor) };
        let Some(view) = (unsafe { ComPtr::<IPlugView>::from_raw(view) }) else {
            return false;
        };
        unsafe { view.isPlatformTypeSupported(PLATFORM_TYPE) == kResultTrue }
    }

    fn arrangement(&self, dir: BusDirection, index: int32) -> SpeakerArrangement {
        let mut arr: SpeakerArrangement = 0;
        unsafe { self.processor.getBusArrangement(dir, index, &mut arr) };
        arr
    }

    /// Sets `arrangement` on the main buses, keeping the others as they are, and returns the
    /// audio input and output bus counts. The plugin must be inactive.
    fn negotiate(&self, arrangement: SpeakerArrangement) -> Result<(int32, int32), InstanceError> {
        let ins = unsafe { self.component.getBusCount(AUDIO, INPUT) };
        let outs = unsafe { self.component.getBusCount(AUDIO, OUTPUT) };
        if ins < 1 || outs < 1 {
            return Err(InstanceError::NotAnEffect);
        }
        let mut in_arr: Vec<_> = (0..ins).map(|i| self.arrangement(INPUT, i)).collect();
        let mut out_arr: Vec<_> = (0..outs).map(|i| self.arrangement(OUTPUT, i)).collect();
        in_arr[0] = arrangement;
        out_arr[0] = arrangement;
        unsafe {
            self.processor
                .setBusArrangements(in_arr.as_mut_ptr(), ins, out_arr.as_mut_ptr(), outs)
        };
        // Plugins may answer kResultFalse yet adapt, or kResultTrue and keep another layout,
        // so the main buses are checked rather than the result.
        if self.arrangement(INPUT, 0) != arrangement || self.arrangement(OUTPUT, 0) != arrangement {
            return Err(InstanceError::Layout);
        }
        Ok((ins, outs))
    }

    /// The plugin's current latency in frames.
    pub fn latency(&self) -> u32 {
        unsafe { self.processor.getLatencySamples() }
    }

    /// Clears the plugin's internal buffers (tails, delay lines) by deactivating and
    /// reactivating it. UI thread.
    pub fn reset(&self) {
        let mut proc = lock(&self.proc);
        if proc.active {
            self.deactivate(&mut proc);
            proc.active = ok(unsafe { self.component.setActive(1) });
        }
    }

    fn deactivate(&self, proc: &mut Processing) {
        unsafe {
            if proc.processing {
                self.processor.setProcessing(0);
                proc.processing = false;
            }
            if proc.active {
                self.component.setActive(0);
                proc.active = false;
            }
        }
    }

    /// Processes `frames` frames from `input` into `output`, one slice per channel, in blocks
    /// of at most the configured size. Pending parameter values are delivered with the first
    /// block. Returns false, leaving `output` untouched, when the instance isn't configured
    /// for these channels or the plugin reports an error.
    pub fn process(
        &self,
        input: &[&[f32]],
        output: &mut [&mut [f32]],
        frames: usize,
        transport: Transport,
    ) -> bool {
        let mut proc = lock(&self.proc);
        let Some(config) = proc.config.filter(|_| proc.active) else {
            return false;
        };
        let channels = config.channels as usize;
        if input.len() != channels
            || output.len() != channels
            || input.iter().any(|c| c.len() < frames)
            || output.iter().any(|c| c.len() < frames)
        {
            return false;
        }
        if !proc.processing {
            unsafe { self.processor.setProcessing(1) };
            proc.processing = true;
        }
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(config.max_block);
            let mut ins: Vec<*mut f32> = input
                .iter()
                .map(|c| c[done..].as_ptr() as *mut f32)
                .collect();
            let mut outs: Vec<*mut f32> =
                output.iter_mut().map(|c| c[done..].as_mut_ptr()).collect();
            let block = Transport {
                position: transport.position + done as i64,
                ..transport
            };
            if !self.process_block(&mut proc, &config, &mut ins, &mut outs, n, block) {
                return false;
            }
            done += n;
        }
        true
    }

    fn process_block(
        &self,
        proc: &mut Processing,
        config: &Config,
        ins: &mut [*mut f32],
        outs: &mut [*mut f32],
        frames: usize,
        transport: Transport,
    ) -> bool {
        proc.input_changes.clear();
        proc.output_changes.clear();
        self.pending.drain_into(&proc.input_changes);

        let silence = proc.silence.as_mut_ptr();
        let scratch = proc.scratch.as_mut_ptr();
        let aux_in: Vec<Vec<*mut f32>> = proc
            .aux_in
            .iter()
            .map(|&n| vec![silence; n as usize])
            .collect();
        let aux_out: Vec<Vec<*mut f32>> = proc
            .aux_out
            .iter()
            .map(|&n| vec![scratch; n as usize])
            .collect();
        let bus = |channels: &[*mut f32]| AudioBusBuffers {
            numChannels: channels.len() as int32,
            silenceFlags: 0,
            __field0: AudioBusBuffers__type0 {
                channelBuffers32: channels.as_ptr() as *mut *mut f32,
            },
        };
        let mut in_buses: Vec<_> = std::iter::once(bus(ins))
            .chain(aux_in.iter().map(|c| bus(c)))
            .collect();
        let mut out_buses: Vec<_> = std::iter::once(bus(outs))
            .chain(aux_out.iter().map(|c| bus(c)))
            .collect();

        let seconds = transport.position as f64 / config.sample_rate;
        let mut context: ProcessContext = unsafe { std::mem::zeroed() };
        context.state = (kTempoValid | kTimeSigValid | kProjectTimeMusicValid) as uint32;
        if transport.playing {
            context.state |= kPlaying as uint32;
        }
        context.sampleRate = config.sample_rate;
        context.projectTimeSamples = transport.position;
        context.continousTimeSamples = transport.position;
        context.tempo = 120.0;
        context.projectTimeMusic = seconds * 2.0;
        context.timeSigNumerator = 4;
        context.timeSigDenominator = 4;

        let as_ptr = |c: &ComWrapper<ParameterChanges>| {
            c.as_com_ref::<IParameterChanges>()
                .map_or(std::ptr::null_mut(), |r| r.as_ptr())
        };
        let events = proc
            .events
            .as_com_ref::<IEventList>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr());
        let mode = if config.offline {
            ProcessModes_::kOffline
        } else {
            ProcessModes_::kRealtime
        };
        let mut data = ProcessData {
            processMode: mode as int32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as int32,
            numSamples: frames as int32,
            numInputs: in_buses.len() as int32,
            numOutputs: out_buses.len() as int32,
            inputs: in_buses.as_mut_ptr(),
            outputs: out_buses.as_mut_ptr(),
            inputParameterChanges: as_ptr(&proc.input_changes),
            outputParameterChanges: as_ptr(&proc.output_changes),
            inputEvents: events,
            outputEvents: events,
            processContext: &mut context,
        };
        ok(unsafe { self.processor.process(&mut data) })
    }

    /// The plugin's state. UI thread.
    pub fn get_state(&self) -> Option<State> {
        let component = ComWrapper::new(MemoryStream::default());
        let s = component.as_com_ref::<IBStream>()?;
        if !ok(unsafe { self.component.getState(s.as_ptr()) }) {
            return None;
        }
        let controller = ComWrapper::new(MemoryStream::default());
        if let (Some(c), Some(s)) = (&self.controller, controller.as_com_ref::<IBStream>())
            && self.separate_controller
        {
            unsafe { c.getState(s.as_ptr()) };
        }
        Some(State {
            component: component.bytes(),
            controller: controller.bytes(),
        })
    }

    /// Restores state from [`Instance::get_state`]. UI thread.
    pub fn set_state(&self, state: &State) -> bool {
        let component = ComWrapper::new(MemoryStream::from_bytes(state.component.clone()));
        let Some(s) = component.as_com_ref::<IBStream>() else {
            return false;
        };
        if !ok(unsafe { self.component.setState(s.as_ptr()) }) {
            return false;
        }
        if let Some(c) = &self.controller
            && self.separate_controller
        {
            component.rewind();
            unsafe { c.setComponentState(s.as_ptr()) };
            let controller = ComWrapper::new(MemoryStream::from_bytes(state.controller.clone()));
            if let Some(s) = controller.as_com_ref::<IBStream>()
                && !state.controller.is_empty()
            {
                unsafe { c.setState(s.as_ptr()) };
            }
        }
        true
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let mut proc = lock(&self.proc);
        self.deactivate(&mut proc);
        drop(proc);
        unsafe {
            if let Some((a, b)) = self.connection.take() {
                a.disconnect(b.as_ptr());
                b.disconnect(a.as_ptr());
            }
            if let Some(c) = &self.controller {
                c.setComponentHandler(std::ptr::null_mut());
                if self.separate_controller {
                    c.terminate();
                }
            }
            self.component.terminate();
        }
    }
}

fn find_bypass_param(controller: &ComPtr<IEditController>) -> Option<ParamID> {
    let count = unsafe { controller.getParameterCount() };
    (0..count).find_map(|i| {
        let mut info: ParameterInfo = unsafe { std::mem::zeroed() };
        let found = ok(unsafe { controller.getParameterInfo(i, &mut info) })
            && info.flags & kIsBypass as int32 != 0;
        found.then_some(info.id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrangements_follow_the_wav_layout() {
        assert_eq!(speaker_arrangement(1, None), SpeakerArr::kMono);
        assert_eq!(speaker_arrangement(1, Some(0x4)), SpeakerArr::kMono);
        assert_eq!(speaker_arrangement(2, None), SpeakerArr::kStereo);
        assert_eq!(speaker_arrangement(2, Some(0x3)), SpeakerArr::kStereo);
        assert_eq!(speaker_arrangement(4, Some(0x33)), SpeakerArr::k40Music);
        assert_eq!(speaker_arrangement(5, Some(0x37)), SpeakerArr::k50);
        assert_eq!(speaker_arrangement(6, Some(0x3F)), SpeakerArr::k51);
        // 5.1 with side surrounds is VST3's 5.1.
        assert_eq!(speaker_arrangement(6, Some(0x60F)), SpeakerArr::k51);
        assert_eq!(speaker_arrangement(8, Some(0x63F)), SpeakerArr::k71Music);
        // A mask that doesn't match the channel count is ignored.
        assert_eq!(speaker_arrangement(6, Some(0x3)), 0x3F);
        assert_eq!(speaker_arrangement(3, None), 0x7);
    }
}
