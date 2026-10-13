//! Host-side COM objects that plugins call back into.
//!
//! The release profile aborts on panic, so these callbacks never unwrap: poisoned locks are
//! recovered and unexpected input returns `kInvalidArgument` or `kResultFalse`.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, Mutex, MutexGuard};

use vst3::Steinberg::IBStream_::{IStreamSeekMode, IStreamSeekMode_};
use vst3::Steinberg::Vst::IAttributeList_::AttrID;
use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{Class, ComWrapper, Interface};

pub const HOST_NAME: &str = "HitherDither";

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn copy_wstring(src: &str, dst: &mut [TChar]) {
    let max = dst.len() - 1;
    let mut n = 0;
    for (d, s) in dst.iter_mut().zip(src.encode_utf16()).take(max) {
        *d = s;
        n += 1;
    }
    dst[n] = 0;
}

/// Hands a new COM object to the caller as `iid` through `obj`.
unsafe fn query<C: Class + 'static>(
    object: ComWrapper<C>,
    iid: *const TUID,
    obj: *mut *mut c_void,
) -> tresult {
    let Some(unknown) = object.to_com_ptr::<FUnknown>() else {
        return kNoInterface;
    };
    let ptr = unknown.as_ptr();
    unsafe { ((*(*ptr).vtbl).queryInterface)(ptr, iid, obj) }
}

fn same_iid(a: *const TUID, iid: &vst3::com_scrape_types::Guid) -> bool {
    !a.is_null() && unsafe { (*a).map(|b| b as u8) } == *iid
}

/// The host context passed to `initialize`.
pub struct HostApplication;

impl Class for HostApplication {
    type Interfaces = (IHostApplication, IPlugInterfaceSupport);
}

impl IHostApplicationTrait for HostApplication {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        if name.is_null() {
            return kInvalidArgument;
        }
        copy_wstring(HOST_NAME, unsafe { &mut *name });
        kResultOk
    }

    unsafe fn createInstance(
        &self,
        cid: *mut TUID,
        iid: *mut TUID,
        obj: *mut *mut c_void,
    ) -> tresult {
        if obj.is_null() {
            return kInvalidArgument;
        }
        unsafe { *obj = std::ptr::null_mut() };
        if same_iid(cid, &IMessage::IID) {
            unsafe { query(ComWrapper::new(Message::default()), iid, obj) }
        } else if same_iid(cid, &IAttributeList::IID) {
            unsafe { query(ComWrapper::new(AttributeList::default()), iid, obj) }
        } else {
            kResultFalse
        }
    }
}

impl IPlugInterfaceSupportTrait for HostApplication {
    unsafe fn isPlugInterfaceSupported(&self, iid: *const TUID) -> tresult {
        let supported = [
            IComponent::IID,
            IAudioProcessor::IID,
            IEditController::IID,
            IConnectionPoint::IID,
            IPlugView::IID,
            IPlugViewContentScaleSupport::IID,
        ];
        if supported.iter().any(|s| same_iid(iid, s)) {
            kResultTrue
        } else {
            kResultFalse
        }
    }
}

/// Parameter values set by the UI that the processor hasn't received yet. The latest value
/// for each parameter wins; the next `process` call delivers them all at offset 0.
#[derive(Default)]
pub struct PendingParams {
    values: Mutex<Vec<(ParamID, f64)>>,
}

impl PendingParams {
    pub fn set(&self, id: ParamID, value: f64) {
        let mut values = lock(&self.values);
        match values.iter_mut().find(|(i, _)| *i == id) {
            Some(entry) => entry.1 = value,
            None => values.push((id, value)),
        }
    }

    /// Moves every pending value into `changes` at offset 0.
    pub fn drain_into(&self, changes: &ParameterChanges) {
        let mut values = lock(&self.values);
        for (id, value) in values.drain(..) {
            changes.push(id, 0, value);
        }
    }

    pub fn is_empty(&self) -> bool {
        lock(&self.values).is_empty()
    }

    /// Every pending value, leaving them pending.
    pub fn values(&self) -> Vec<(ParamID, f64)> {
        lock(&self.values).clone()
    }

    /// Takes every pending value.
    pub fn take(&self) -> Vec<(ParamID, f64)> {
        std::mem::take(&mut *lock(&self.values))
    }
}

/// What a plugin's controller tells the host, besides parameter values it already wrote to
/// [`PendingParams`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HandlerEvent {
    /// A parameter changed from the plugin's own editor.
    Edited { id: ParamID, value: f64 },
    /// `restartComponent` with these `RestartFlags`.
    Restart(i32),
    /// The processor reported output parameter values (such as meters), waiting in the
    /// instance's [`outputs`](super::instance::Instance::outputs). Sent from the processing
    /// thread.
    Output,
}

pub type HandlerListener = Box<dyn Fn(HandlerEvent) + Send + Sync>;

/// Receives edits and restart requests from a controller.
pub struct ComponentHandler {
    pending: Arc<PendingParams>,
    listener: Mutex<Option<HandlerListener>>,
}

impl ComponentHandler {
    pub fn new(pending: Arc<PendingParams>) -> ComponentHandler {
        ComponentHandler {
            pending,
            listener: Mutex::new(None),
        }
    }

    /// Sets the callback for events; it runs on whichever thread the plugin calls from
    /// (normally the UI thread) and must not block.
    pub fn set_listener(&self, listener: Option<HandlerListener>) {
        *lock(&self.listener) = listener;
    }

    pub(super) fn notify(&self, event: HandlerEvent) {
        if let Some(l) = lock(&self.listener).as_ref() {
            l(event);
        }
    }
}

impl Class for ComponentHandler {
    type Interfaces = (IComponentHandler, IComponentHandler2);
}

impl IComponentHandlerTrait for ComponentHandler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        self.pending.set(id, value);
        self.notify(HandlerEvent::Edited { id, value });
        kResultOk
    }

    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, flags: int32) -> tresult {
        self.notify(HandlerEvent::Restart(flags));
        kResultOk
    }
}

impl IComponentHandler2Trait for ComponentHandler {
    unsafe fn setDirty(&self, _state: TBool) -> tresult {
        kResultOk
    }

    unsafe fn requestOpenEditor(&self, _name: FIDString) -> tresult {
        kResultFalse
    }

    unsafe fn startGroupEdit(&self) -> tresult {
        kResultOk
    }

    unsafe fn finishGroupEdit(&self) -> tresult {
        kResultOk
    }
}

/// Resizes the window holding a plugin view; returns whether it did.
pub type ResizeFn = Box<dyn Fn(*mut IPlugView, ViewRect) -> bool + Send + Sync>;

/// The frame given to a plugin view, which asks the host to resize its window through it.
pub struct PlugFrame {
    resize: ResizeFn,
}

impl PlugFrame {
    pub fn new(resize: ResizeFn) -> PlugFrame {
        PlugFrame { resize }
    }
}

impl Class for PlugFrame {
    type Interfaces = (IPlugFrame,);
}

impl IPlugFrameTrait for PlugFrame {
    unsafe fn resizeView(&self, view: *mut IPlugView, new_size: *mut ViewRect) -> tresult {
        if view.is_null() || new_size.is_null() {
            return kInvalidArgument;
        }
        if (self.resize)(view, unsafe { *new_size }) {
            kResultOk
        } else {
            kResultFalse
        }
    }
}

/// One parameter's points within a block.
#[derive(Default)]
pub struct ParamValueQueue {
    data: Mutex<(ParamID, Vec<(int32, ParamValue)>)>,
}

impl Class for ParamValueQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParamValueQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        lock(&self.data).0
    }

    unsafe fn getPointCount(&self) -> int32 {
        lock(&self.data).1.len() as int32
    }

    unsafe fn getPoint(&self, index: int32, offset: *mut int32, value: *mut ParamValue) -> tresult {
        let data = lock(&self.data);
        match usize::try_from(index).ok().and_then(|i| data.1.get(i)) {
            Some(&(o, v)) if !offset.is_null() && !value.is_null() => {
                unsafe {
                    *offset = o;
                    *value = v;
                }
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn addPoint(&self, offset: int32, value: ParamValue, index: *mut int32) -> tresult {
        let mut data = lock(&self.data);
        let points = &mut data.1;
        // Points are kept in offset order; a point at an existing offset replaces it.
        let at = points.partition_point(|&(o, _)| o < offset);
        if points.get(at).is_some_and(|&(o, _)| o == offset) {
            points[at].1 = value;
        } else {
            points.insert(at, (offset, value));
        }
        if !index.is_null() {
            unsafe { *index = at as int32 };
        }
        kResultOk
    }
}

/// Parameter changes for one block, input or output. Queue objects are pooled and reused
/// from block to block.
#[derive(Default)]
pub struct ParameterChanges {
    queues: Mutex<(Vec<ComWrapper<ParamValueQueue>>, usize)>,
}

impl ParameterChanges {
    pub fn clear(&self) {
        lock(&self.queues).1 = 0;
    }

    /// Adds a point, creating the parameter's queue if needed.
    pub fn push(&self, id: ParamID, offset: i32, value: f64) {
        let queue = self.queue_for(id);
        unsafe { queue.addPoint(offset, value, std::ptr::null_mut()) };
    }

    /// The last value of each parameter in these changes.
    pub fn last_values(&self) -> Vec<(ParamID, f64)> {
        let queues = lock(&self.queues);
        queues.0[..queues.1]
            .iter()
            .filter_map(|q| {
                let data = lock(&q.data);
                data.1.last().map(|&(_, v)| (data.0, v))
            })
            .collect()
    }

    /// Sets the last value of each parameter in these changes in `target`, without
    /// allocating. Returns whether there were any.
    pub fn copy_last_into(&self, target: &PendingParams) -> bool {
        let queues = lock(&self.queues);
        let mut any = false;
        for q in &queues.0[..queues.1] {
            let data = lock(&q.data);
            if let Some(&(_, v)) = data.1.last() {
                target.set(data.0, v);
                any = true;
            }
        }
        any
    }

    fn queue_for(&self, id: ParamID) -> ComWrapper<ParamValueQueue> {
        let mut queues = lock(&self.queues);
        let (pool, used) = &mut *queues;
        if let Some(q) = pool[..*used].iter().find(|q| lock(&q.data).0 == id) {
            return q.clone();
        }
        if *used == pool.len() {
            pool.push(ComWrapper::new(ParamValueQueue::default()));
        }
        let q = pool[*used].clone();
        *lock(&q.data) = (id, Vec::new());
        *used += 1;
        q
    }
}

impl Class for ParameterChanges {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for ParameterChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        lock(&self.queues).1 as int32
    }

    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        let queues = lock(&self.queues);
        match usize::try_from(index) {
            Ok(i) if i < queues.1 => queue_ptr(&queues.0[i]),
            _ => std::ptr::null_mut(),
        }
    }

    unsafe fn addParameterData(
        &self,
        id: *const ParamID,
        index: *mut int32,
    ) -> *mut IParamValueQueue {
        if id.is_null() {
            return std::ptr::null_mut();
        }
        let queue = self.queue_for(unsafe { *id });
        if !index.is_null() {
            let queues = lock(&self.queues);
            let at = queues.0[..queues.1]
                .iter()
                .position(|q| queue_ptr(q) == queue_ptr(&queue));
            unsafe { *index = at.map_or(-1, |i| i as int32) };
        }
        queue_ptr(&queue)
    }
}

/// The queue's interface pointer, which stays valid while the pool holds the object.
fn queue_ptr(queue: &ComWrapper<ParamValueQueue>) -> *mut IParamValueQueue {
    queue
        .as_com_ref::<IParamValueQueue>()
        .map_or(std::ptr::null_mut(), |r| r.as_ptr())
}

/// An event list that is always empty and drops events the plugin adds: effects get no notes.
pub struct EventList;

impl Class for EventList {
    type Interfaces = (IEventList,);
}

impl IEventListTrait for EventList {
    unsafe fn getEventCount(&self) -> int32 {
        0
    }

    unsafe fn getEvent(&self, _index: int32, _event: *mut Event) -> tresult {
        kInvalidArgument
    }

    unsafe fn addEvent(&self, _event: *mut Event) -> tresult {
        kResultOk
    }
}

/// A growable in-memory stream for plugin state.
#[derive(Default)]
pub struct MemoryStream {
    data: Mutex<(Vec<u8>, usize)>,
}

impl MemoryStream {
    pub fn from_bytes(bytes: Vec<u8>) -> MemoryStream {
        MemoryStream {
            data: Mutex::new((bytes, 0)),
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        lock(&self.data).0.clone()
    }

    pub fn rewind(&self) {
        lock(&self.data).1 = 0;
    }
}

impl Class for MemoryStream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for MemoryStream {
    unsafe fn read(&self, buffer: *mut c_void, num_bytes: int32, read: *mut int32) -> tresult {
        if buffer.is_null() || num_bytes < 0 {
            return kInvalidArgument;
        }
        let mut data = lock(&self.data);
        let (bytes, pos) = &mut *data;
        let n = (num_bytes as usize).min(bytes.len().saturating_sub(*pos));
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().add(*pos), buffer.cast(), n) };
        *pos += n;
        if !read.is_null() {
            unsafe { *read = n as int32 };
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, num_bytes: int32, written: *mut int32) -> tresult {
        if buffer.is_null() || num_bytes < 0 {
            return kInvalidArgument;
        }
        let mut data = lock(&self.data);
        let (bytes, pos) = &mut *data;
        let n = num_bytes as usize;
        let end = *pos + n;
        if bytes.len() < end {
            bytes.resize(end, 0);
        }
        let src = unsafe { std::slice::from_raw_parts(buffer.cast::<u8>(), n) };
        bytes[*pos..end].copy_from_slice(src);
        *pos = end;
        if !written.is_null() {
            unsafe { *written = n as int32 };
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let mut data = lock(&self.data);
        let base = match mode as IStreamSeekMode {
            IStreamSeekMode_::kIBSeekSet => 0,
            IStreamSeekMode_::kIBSeekCur => data.1 as i64,
            IStreamSeekMode_::kIBSeekEnd => data.0.len() as i64,
            _ => return kInvalidArgument,
        };
        let Some(target) = base.checked_add(pos).filter(|&t| t >= 0) else {
            return kInvalidArgument;
        };
        data.1 = target as usize;
        if !result.is_null() {
            unsafe { *result = target };
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        if pos.is_null() {
            return kInvalidArgument;
        }
        unsafe { *pos = lock(&self.data).1 as int64 };
        kResultOk
    }
}

#[derive(Clone)]
enum Attr {
    Int(i64),
    Float(f64),
    String(Vec<TChar>),
    Binary(Vec<u8>),
}

/// Attributes of a message between a plugin's component and controller.
#[derive(Default)]
pub struct AttributeList {
    attrs: Mutex<HashMap<CString, Attr>>,
}

impl AttributeList {
    fn set(&self, id: AttrID, value: Attr) -> tresult {
        if id.is_null() {
            return kInvalidArgument;
        }
        let key = unsafe { CStr::from_ptr(id) }.to_owned();
        lock(&self.attrs).insert(key, value);
        kResultOk
    }

    fn get(&self, id: AttrID) -> Option<Attr> {
        if id.is_null() {
            return None;
        }
        lock(&self.attrs)
            .get(unsafe { CStr::from_ptr(id) })
            .cloned()
    }
}

impl Class for AttributeList {
    type Interfaces = (IAttributeList,);
}

impl IAttributeListTrait for AttributeList {
    unsafe fn setInt(&self, id: AttrID, value: int64) -> tresult {
        self.set(id, Attr::Int(value))
    }

    unsafe fn getInt(&self, id: AttrID, value: *mut int64) -> tresult {
        match self.get(id) {
            Some(Attr::Int(v)) if !value.is_null() => {
                unsafe { *value = v };
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setFloat(&self, id: AttrID, value: f64) -> tresult {
        self.set(id, Attr::Float(value))
    }

    unsafe fn getFloat(&self, id: AttrID, value: *mut f64) -> tresult {
        match self.get(id) {
            Some(Attr::Float(v)) if !value.is_null() => {
                unsafe { *value = v };
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setString(&self, id: AttrID, string: *const TChar) -> tresult {
        if string.is_null() {
            return kInvalidArgument;
        }
        let mut chars = Vec::new();
        let mut i = 0;
        loop {
            let c = unsafe { *string.add(i) };
            if c == 0 {
                break;
            }
            chars.push(c);
            i += 1;
        }
        self.set(id, Attr::String(chars))
    }

    unsafe fn getString(&self, id: AttrID, string: *mut TChar, size_in_bytes: uint32) -> tresult {
        let capacity = size_in_bytes as usize / size_of::<TChar>();
        match self.get(id) {
            Some(Attr::String(chars)) if !string.is_null() && capacity > 0 => {
                let n = chars.len().min(capacity - 1);
                unsafe {
                    std::ptr::copy_nonoverlapping(chars.as_ptr(), string, n);
                    *string.add(n) = 0;
                }
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setBinary(&self, id: AttrID, data: *const c_void, size: uint32) -> tresult {
        if data.is_null() && size > 0 {
            return kInvalidArgument;
        }
        let bytes = if size == 0 {
            Vec::new()
        } else {
            unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) }.to_vec()
        };
        self.set(id, Attr::Binary(bytes))
    }

    unsafe fn getBinary(&self, id: AttrID, data: *mut *const c_void, size: *mut uint32) -> tresult {
        if id.is_null() || data.is_null() || size.is_null() {
            return kInvalidArgument;
        }
        // The pointer must stay valid after the lock is released, so hand out the stored
        // buffer itself; it lives until the attribute is set again or the list is released.
        let attrs = lock(&self.attrs);
        match attrs.get(unsafe { CStr::from_ptr(id) }) {
            Some(Attr::Binary(bytes)) => {
                unsafe {
                    *data = bytes.as_ptr().cast();
                    *size = bytes.len() as uint32;
                }
                kResultOk
            }
            _ => kResultFalse,
        }
    }
}

/// A message between a plugin's component and controller, created through
/// `IHostApplication::createInstance`.
pub struct Message {
    id: Mutex<Option<CString>>,
    attributes: ComWrapper<AttributeList>,
}

impl Default for Message {
    fn default() -> Message {
        Message {
            id: Mutex::new(None),
            attributes: ComWrapper::new(AttributeList::default()),
        }
    }
}

impl Class for Message {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> FIDString {
        lock(&self.id)
            .as_ref()
            .map_or(std::ptr::null(), |id| id.as_ptr())
    }

    unsafe fn setMessageID(&self, id: FIDString) {
        *lock(&self.id) =
            (!id.is_null()).then(|| unsafe { CStr::from_ptr(id as *const c_char) }.to_owned());
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes
            .as_com_ref::<IAttributeList>()
            .map_or(std::ptr::null_mut(), |r| r.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vst3::ComPtr;

    #[test]
    fn memory_stream_reads_back_what_was_written() {
        let s = ComWrapper::new(MemoryStream::default());
        let mut n = 0;
        unsafe {
            s.write(b"hello".as_ptr() as *mut c_void, 5, &mut n);
            assert_eq!(n, 5);
            let mut pos = 0;
            s.seek(1, IStreamSeekMode_::kIBSeekSet as int32, &mut pos);
            let mut buf = [0u8; 8];
            s.read(buf.as_mut_ptr().cast(), 8, &mut n);
            assert_eq!((n, &buf[..4]), (4, &b"ello"[..]));
            s.tell(&mut pos);
            assert_eq!(pos, 5);
        }
        assert_eq!(s.bytes(), b"hello");
    }

    #[test]
    fn parameter_changes_keep_one_queue_per_parameter() {
        let changes = ComWrapper::new(ParameterChanges::default());
        changes.push(3, 0, 0.25);
        changes.push(5, 0, 0.5);
        changes.push(3, 10, 0.75);
        let ptr: ComPtr<IParameterChanges> = changes.to_com_ptr().unwrap();
        unsafe {
            assert_eq!(ptr.getParameterCount(), 2);
            let q = vst3::ComRef::from_raw(ptr.getParameterData(0)).unwrap();
            assert_eq!((q.getParameterId(), q.getPointCount()), (3, 2));
        }
        assert_eq!(changes.last_values(), vec![(3, 0.75), (5, 0.5)]);
        changes.clear();
        assert_eq!(unsafe { ptr.getParameterCount() }, 0);
        assert!(changes.last_values().is_empty());
    }

    #[test]
    fn pending_params_keep_the_latest_value() {
        let pending = PendingParams::default();
        pending.set(1, 0.1);
        pending.set(1, 0.9);
        let changes = ParameterChanges::default();
        pending.drain_into(&changes);
        assert_eq!(changes.last_values(), vec![(1, 0.9)]);
        assert!(pending.is_empty());
    }

    #[test]
    fn host_creates_messages_with_attributes() {
        let host = ComWrapper::new(HostApplication);
        let mut obj = std::ptr::null_mut();
        let mut cid = IMessage::IID.map(|b| b as i8);
        let mut iid = cid;
        assert_eq!(
            unsafe { host.createInstance(&mut cid, &mut iid, &mut obj) },
            kResultOk
        );
        let msg = unsafe { ComPtr::from_raw(obj.cast::<IMessage>()) }.unwrap();
        unsafe {
            msg.setMessageID(c"hello".as_ptr());
            assert_eq!(CStr::from_ptr(msg.getMessageID()), c"hello");
            let attrs = vst3::ComRef::from_raw(msg.getAttributes()).unwrap();
            attrs.setInt(c"n".as_ptr(), 7);
            let mut v = 0;
            assert_eq!(attrs.getInt(c"n".as_ptr(), &mut v), kResultOk);
            assert_eq!(v, 7);
            let blob = [1u8, 2, 3];
            attrs.setBinary(c"b".as_ptr(), blob.as_ptr().cast(), 3);
            let (mut p, mut len) = (std::ptr::null(), 0);
            attrs.getBinary(c"b".as_ptr(), &mut p, &mut len);
            assert_eq!(
                std::slice::from_raw_parts(p.cast::<u8>(), len as usize),
                &blob
            );
        }
    }
}
