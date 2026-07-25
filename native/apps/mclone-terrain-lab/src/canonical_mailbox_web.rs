use js_sys::{Int32Array, Object, Reflect, SharedArrayBuffer, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};

use crate::canonical_batch_codec::patch_canonical_batch_transfer_ms;

pub(crate) const CANONICAL_SHARED_RESULT_CONTROL_WORDS: u32 = 4;
pub(crate) const CANONICAL_SHARED_RESULT_STATUS_INDEX: u32 = 0;
pub(crate) const CANONICAL_SHARED_RESULT_BYTES_INDEX: u32 = 1;
pub(crate) const CANONICAL_SHARED_RESULT_CAPACITY_INDEX: u32 = 2;
pub(crate) const CANONICAL_SHARED_RESULT_EPOCH_INDEX: u32 = 3;
pub(crate) const CANONICAL_SHARED_RESULT_PENDING: i32 = 1;
pub(crate) const CANONICAL_SHARED_RESULT_COMPLETE: i32 = 2;
pub(crate) const CANONICAL_SHARED_RESULT_OVERFLOW: i32 = 3;
pub(crate) const CANONICAL_SHARED_RESULT_FAILED: i32 = 4;
pub(crate) const CANONICAL_SHARED_RESULT_DEFAULT_CAPACITY: u32 = 16 * 1024 * 1024;
pub(crate) const CANONICAL_SHARED_RESULT_MAX_CAPACITY: u32 = 128 * 1024 * 1024;
pub(crate) const CANONICAL_SHARED_RESULT_TRANSPORT_KIND: &str = "external-shared-result";

pub(crate) struct CanonicalSharedResultArena {
    control_buffer: SharedArrayBuffer,
    control: Int32Array,
    result_buffer: SharedArrayBuffer,
    capacity: u32,
    high_water: u32,
    overflow_count: u32,
}

pub(crate) struct CanonicalSharedResultRead {
    pub bytes: Vec<u8>,
    pub capacity: u32,
    pub high_water: u32,
    pub overflow_count: u32,
}

pub(crate) struct CanonicalSharedPublication {
    pub buffer: SharedArrayBuffer,
    pub byte_length: u32,
    pub capacity: u32,
    pub overflow: bool,
    pub transfer_ms: f64,
}

impl CanonicalSharedResultArena {
    pub(crate) fn new() -> Result<Self, String> {
        if !shared_memory_supported() {
            return Err(
                "Terrain Lab exact terrain requires cross-origin-isolated SharedArrayBuffer"
                    .to_owned(),
            );
        }
        let control_buffer = SharedArrayBuffer::new(CANONICAL_SHARED_RESULT_CONTROL_WORDS * 4);
        let control = Int32Array::new(control_buffer.as_ref());
        let capacity = CANONICAL_SHARED_RESULT_DEFAULT_CAPACITY;
        let result_buffer = SharedArrayBuffer::new(capacity);
        let arena = Self {
            control_buffer,
            control,
            result_buffer,
            capacity,
            high_water: 0,
            overflow_count: 0,
        };
        atomic_store(
            &arena.control,
            CANONICAL_SHARED_RESULT_STATUS_INDEX,
            CANONICAL_SHARED_RESULT_PENDING,
        )?;
        atomic_store(&arena.control, CANONICAL_SHARED_RESULT_BYTES_INDEX, 0)?;
        atomic_store(
            &arena.control,
            CANONICAL_SHARED_RESULT_CAPACITY_INDEX,
            capacity as i32,
        )?;
        atomic_store(&arena.control, CANONICAL_SHARED_RESULT_EPOCH_INDEX, 0)?;
        Ok(arena)
    }

    pub(crate) fn arm_and_attach(&self, frame: &Object, epoch: u32) -> Result<(), String> {
        let epoch = i32::try_from(epoch)
            .map_err(|_| format!("canonical shared-result epoch {epoch} exceeds i32"))?;
        atomic_store(&self.control, CANONICAL_SHARED_RESULT_BYTES_INDEX, 0)?;
        atomic_store(
            &self.control,
            CANONICAL_SHARED_RESULT_CAPACITY_INDEX,
            self.capacity as i32,
        )?;
        atomic_store(&self.control, CANONICAL_SHARED_RESULT_EPOCH_INDEX, epoch)?;
        atomic_store(
            &self.control,
            CANONICAL_SHARED_RESULT_STATUS_INDEX,
            CANONICAL_SHARED_RESULT_PENDING,
        )?;
        set_value(
            frame,
            "sharedResultControlBuffer",
            self.control_buffer.as_ref(),
        )?;
        set_value(
            frame,
            "sharedResultResponseBuffer",
            self.result_buffer.as_ref(),
        )?;
        Ok(())
    }

    pub(crate) fn read_published(
        &mut self,
        response: &JsValue,
        epoch: u32,
    ) -> Result<CanonicalSharedResultRead, String> {
        let status = atomic_load(&self.control, CANONICAL_SHARED_RESULT_STATUS_INDEX)?;
        let published_epoch = atomic_load(&self.control, CANONICAL_SHARED_RESULT_EPOCH_INDEX)?;
        if published_epoch
            != i32::try_from(epoch)
                .map_err(|_| format!("canonical shared-result epoch {epoch} exceeds i32"))?
        {
            return Err(format!(
                "canonical shared-result epoch {published_epoch} does not match {epoch}"
            ));
        }
        let byte_length = nonnegative_control_word(
            &self.control,
            CANONICAL_SHARED_RESULT_BYTES_INDEX,
            "byte length",
        )?;
        let published_capacity = nonnegative_control_word(
            &self.control,
            CANONICAL_SHARED_RESULT_CAPACITY_INDEX,
            "capacity",
        )?;
        let buffer = match status {
            CANONICAL_SHARED_RESULT_COMPLETE => self.result_buffer.clone(),
            CANONICAL_SHARED_RESULT_OVERFLOW => {
                self.overflow_count = self.overflow_count.saturating_add(1);
                let overflow_buffer = shared_array_buffer_property(response, "sharedResultBuffer")?;
                if overflow_buffer.byte_length() < byte_length {
                    return Err(format!(
                        "canonical overflow buffer has {} bytes for a {byte_length}-byte result",
                        overflow_buffer.byte_length()
                    ));
                }
                self.grow_after_overflow(byte_length)?;
                overflow_buffer
            }
            CANONICAL_SHARED_RESULT_FAILED => {
                return Err("canonical Worker marked the shared result as failed".to_owned());
            }
            other => {
                return Err(format!(
                    "canonical shared-result status {other} is not complete"
                ));
            }
        };
        if byte_length > buffer.byte_length() {
            return Err(format!(
                "canonical shared-result byte length {byte_length} exceeds buffer {}",
                buffer.byte_length()
            ));
        }
        if published_capacity < byte_length {
            return Err(format!(
                "canonical shared-result capacity {published_capacity} is below byte length {byte_length}"
            ));
        }
        self.high_water = self.high_water.max(byte_length);
        let bytes = if byte_length == 0 {
            Vec::new()
        } else {
            Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, byte_length).to_vec()
        };
        Ok(CanonicalSharedResultRead {
            bytes,
            capacity: self.capacity,
            high_water: self.high_water,
            overflow_count: self.overflow_count,
        })
    }

    pub(crate) const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub(crate) const fn high_water(&self) -> u32 {
        self.high_water
    }

    pub(crate) const fn overflow_count(&self) -> u32 {
        self.overflow_count
    }

    fn grow_after_overflow(&mut self, required: u32) -> Result<(), String> {
        if required <= self.capacity {
            return Ok(());
        }
        let capacity = required
            .checked_next_power_of_two()
            .unwrap_or(CANONICAL_SHARED_RESULT_MAX_CAPACITY);
        if capacity > CANONICAL_SHARED_RESULT_MAX_CAPACITY || required > capacity {
            return Err(format!(
                "canonical shared result requires {required} bytes, above the {}-byte bound",
                CANONICAL_SHARED_RESULT_MAX_CAPACITY
            ));
        }
        self.result_buffer = SharedArrayBuffer::new(capacity);
        self.capacity = capacity;
        Ok(())
    }
}

pub(crate) fn publish_canonical_shared_result(
    frame: &JsValue,
    epoch: u32,
    encoded: &mut [u8],
    transfer_started_ms: f64,
) -> Result<CanonicalSharedPublication, String> {
    if !shared_memory_supported() {
        return Err("canonical Worker has no SharedArrayBuffer/Atomics support".to_owned());
    }
    let control_buffer = shared_array_buffer_property(frame, "sharedResultControlBuffer")?;
    if control_buffer.byte_length() < CANONICAL_SHARED_RESULT_CONTROL_WORDS * 4 {
        return Err("canonical shared-result control buffer is too small".to_owned());
    }
    let control = Int32Array::new(control_buffer.as_ref());
    if atomic_load(&control, CANONICAL_SHARED_RESULT_STATUS_INDEX)?
        != CANONICAL_SHARED_RESULT_PENDING
    {
        return Err("canonical shared-result mailbox was not armed as pending".to_owned());
    }
    let published_epoch = atomic_load(&control, CANONICAL_SHARED_RESULT_EPOCH_INDEX)?;
    if published_epoch
        != i32::try_from(epoch)
            .map_err(|_| format!("canonical shared-result epoch {epoch} exceeds i32"))?
    {
        return Err(format!(
            "canonical shared-result control epoch {published_epoch} does not match {epoch}"
        ));
    }
    let provided = shared_array_buffer_property(frame, "sharedResultResponseBuffer")?;
    let byte_length = u32::try_from(encoded.len())
        .map_err(|_| "canonical shared result exceeds the u32 byte-length contract".to_owned())?;
    if byte_length > CANONICAL_SHARED_RESULT_MAX_CAPACITY {
        return Err(format!(
            "canonical shared result requires {byte_length} bytes, above the {}-byte bound",
            CANONICAL_SHARED_RESULT_MAX_CAPACITY
        ));
    }
    let overflow = provided.byte_length() < byte_length;
    let buffer = if overflow {
        SharedArrayBuffer::new(byte_length)
    } else {
        provided
    };
    if byte_length > 0 {
        Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 0, byte_length)
            .copy_from(encoded);
    }
    let transfer_ms = js_sys::Date::now() - transfer_started_ms;
    patch_canonical_batch_transfer_ms(encoded, transfer_ms)?;
    Uint8Array::new_with_byte_offset_and_length(buffer.as_ref(), 40, 8)
        .copy_from(&transfer_ms.to_le_bytes());
    atomic_store(
        &control,
        CANONICAL_SHARED_RESULT_BYTES_INDEX,
        byte_length as i32,
    )?;
    atomic_store(
        &control,
        CANONICAL_SHARED_RESULT_CAPACITY_INDEX,
        buffer.byte_length() as i32,
    )?;
    atomic_store(
        &control,
        CANONICAL_SHARED_RESULT_STATUS_INDEX,
        if overflow {
            CANONICAL_SHARED_RESULT_OVERFLOW
        } else {
            CANONICAL_SHARED_RESULT_COMPLETE
        },
    )?;
    atomic_notify(&control, CANONICAL_SHARED_RESULT_STATUS_INDEX)?;
    Ok(CanonicalSharedPublication {
        buffer,
        byte_length,
        capacity: byte_length.max(nonnegative_control_word(
            &control,
            CANONICAL_SHARED_RESULT_CAPACITY_INDEX,
            "capacity",
        )?),
        overflow,
        transfer_ms,
    })
}

pub(crate) fn mark_canonical_shared_result_failed(frame: &JsValue) {
    let Ok(buffer) = shared_array_buffer_property(frame, "sharedResultControlBuffer") else {
        return;
    };
    if buffer.byte_length() < CANONICAL_SHARED_RESULT_CONTROL_WORDS * 4 {
        return;
    }
    let control = Int32Array::new(buffer.as_ref());
    let _ = atomic_store(
        &control,
        CANONICAL_SHARED_RESULT_STATUS_INDEX,
        CANONICAL_SHARED_RESULT_FAILED,
    );
    let _ = atomic_notify(&control, CANONICAL_SHARED_RESULT_STATUS_INDEX);
}

pub(crate) fn shared_memory_supported() -> bool {
    let global = js_sys::global();
    global_function(&global, "SharedArrayBuffer")
        && global_method(&global, "Atomics", "load")
        && global_method(&global, "Atomics", "store")
        && global_method(&global, "Atomics", "notify")
        && Reflect::get(&global, &JsValue::from_str("crossOriginIsolated"))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
}

fn global_function(global: &JsValue, name: &str) -> bool {
    Reflect::get(global, &JsValue::from_str(name))
        .ok()
        .is_some_and(|value| value.is_function())
}

fn global_method(global: &JsValue, object: &str, name: &str) -> bool {
    Reflect::get(global, &JsValue::from_str(object))
        .ok()
        .and_then(|object| Reflect::get(&object, &JsValue::from_str(name)).ok())
        .is_some_and(|value| value.is_function())
}

fn atomic_store(control: &Int32Array, index: u32, value: i32) -> Result<(), String> {
    js_sys::Atomics::store(control, index, value)
        .map(|_| ())
        .map_err(|error| format!("failed to store canonical shared-result word {index}: {error:?}"))
}

fn atomic_load(control: &Int32Array, index: u32) -> Result<i32, String> {
    js_sys::Atomics::load(control, index)
        .map_err(|error| format!("failed to load canonical shared-result word {index}: {error:?}"))
}

fn atomic_notify(control: &Int32Array, index: u32) -> Result<(), String> {
    js_sys::Atomics::notify_with_count(control, index, 1)
        .map(|_| ())
        .map_err(|error| {
            format!("failed to notify canonical shared-result word {index}: {error:?}")
        })
}

fn nonnegative_control_word(control: &Int32Array, index: u32, label: &str) -> Result<u32, String> {
    let value = atomic_load(control, index)?;
    u32::try_from(value)
        .map_err(|_| format!("canonical shared-result {label} is negative ({value})"))
}

fn shared_array_buffer_property(value: &JsValue, name: &str) -> Result<SharedArrayBuffer, String> {
    Reflect::get(value, &JsValue::from_str(name))
        .map_err(js_message)?
        .dyn_into::<SharedArrayBuffer>()
        .map_err(|_| format!("canonical Worker property {name:?} is not a SharedArrayBuffer"))
}

fn set_value(target: &Object, name: &str, value: &JsValue) -> Result<(), String> {
    Reflect::set(target, &JsValue::from_str(name), value)
        .map(|_| ())
        .map_err(js_message)
}

fn js_message(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            Reflect::get(&value, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| "canonical shared-result operation failed".to_owned())
}
