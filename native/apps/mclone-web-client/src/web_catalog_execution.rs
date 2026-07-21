//! Browser adapter for the shared world-catalog storage-plan continuation.
//!
//! Shared Rust owns catalog policy and transaction sequencing. This module
//! maps stable storage-plan values to `JsValue`, decodes IndexedDB reads, and
//! manages browser writer leases without retaining Rust borrows across an
//! asynchronous gap.

#[cfg(target_arch = "wasm32")]
use mclone_app_runtime::catalog_storage_plan::{
    CatalogExecutionCore, CatalogReadResult, CatalogReadShape, StorageAction, StorageActionKind,
    StorageStep,
};
#[cfg(target_arch = "wasm32")]
use mclone_app_runtime::platform_operation::PlatformOperationToken;
#[cfg(target_arch = "wasm32")]
use mclone_app_runtime::world_catalog::{LocalWorldId, WorldCatalogRequest, WorldCatalogResponse};

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) const WEB_WORLD_BACKEND_LABEL: &str = "web-indexeddb";

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn web_world_writer_lease_name(world_id: &str) -> String {
    format!("mclone:indexeddb-world-writer:{world_id}")
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use js_sys::{Array, Object, Reflect};
    use wasm_bindgen::prelude::*;

    use super::*;
    use crate::web_canvas::{
        decode_web_local_world_create_options, decode_web_local_world_summaries,
        decode_web_local_world_summary, encode_web_catalog_record, encode_web_local_world_summary,
        parse_web_unix_millis,
    };

    #[wasm_bindgen]
    pub struct WebCatalogExecution {
        token: Option<PlatformOperationToken>,
        core: CatalogExecutionCore,
    }

    impl WebCatalogExecution {
        pub(crate) fn new(
            token: PlatformOperationToken,
            request: WorldCatalogRequest,
            active_world: Option<LocalWorldId>,
        ) -> Result<Self, String> {
            Ok(Self {
                token: Some(token),
                core: CatalogExecutionCore::new(request, active_world, WEB_WORLD_BACKEND_LABEL)?,
            })
        }

        pub(crate) fn token(&self) -> Result<PlatformOperationToken, String> {
            self.token
                .ok_or_else(|| "catalog smoke execution has no platform-operation token".to_owned())
        }

        pub(crate) fn response(&self) -> Result<WorldCatalogResponse, String> {
            self.core.response().cloned()
        }

        pub(crate) fn writer_lease_names(&self) -> Vec<String> {
            self.core
                .required_writer_world_ids()
                .into_iter()
                .map(|id| web_world_writer_lease_name(id.as_str()))
                .collect()
        }
    }

    #[wasm_bindgen]
    impl WebCatalogExecution {
        #[wasm_bindgen(js_name = requiredWriterLeaseNames)]
        pub fn required_writer_lease_names(&self) -> Array {
            self.writer_lease_names()
                .into_iter()
                .map(JsValue::from)
                .collect()
        }

        #[wasm_bindgen(js_name = awaitWriterRetirements)]
        pub async fn await_writer_retirements(&self) -> Result<(), JsValue> {
            for lease_name in self.writer_lease_names() {
                crate::web_server_worker::await_retired_world_writer(&lease_name)
                    .await
                    .map_err(JsValue::from)?;
            }
            Ok(())
        }

        #[wasm_bindgen(js_name = nextStorageStep)]
        pub fn next_storage_step(&mut self) -> Result<JsValue, JsValue> {
            self.core
                .next_step()
                .and_then(|step| step.map(encode_storage_step).transpose())
                .map(|step| step.unwrap_or(JsValue::UNDEFINED))
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = acceptStorageRead)]
        pub fn accept_storage_read(
            &mut self,
            step_id: f64,
            action_id: f64,
            value: JsValue,
            now_unix_millis: f64,
        ) -> Result<Array, JsValue> {
            let step_id = exact_u32(step_id, "catalog storage step id")?;
            let action_id = exact_u32(action_id, "catalog storage action id")?;
            let shape = self
                .core
                .pending_read_shape(step_id, action_id)
                .map_err(|error| JsValue::from_str(&error))?;
            let result = match shape {
                CatalogReadShape::All => {
                    CatalogReadResult::All(decode_web_local_world_summaries(&value).map_err(
                        |error| JsValue::from_str(&format!("decode catalog storage rows: {error}")),
                    )?)
                }
                CatalogReadShape::One => {
                    let summary = if value.is_null() || value.is_undefined() {
                        None
                    } else {
                        Some(decode_web_local_world_summary(&value).map_err(|error| {
                            JsValue::from_str(&format!("decode catalog storage row: {error}"))
                        })?)
                    };
                    CatalogReadResult::One(summary)
                }
            };
            let timestamp = self
                .core
                .pending_read_needs_timestamp(step_id, action_id)
                .map_err(|error| JsValue::from_str(&error))?
                .then(|| parse_web_unix_millis(now_unix_millis, "nowUnixMillis"))
                .transpose()
                .map_err(|error| JsValue::from_str(&error))?;
            let followups = self
                .core
                .accept_read_result(step_id, action_id, result, timestamp)
                .map_err(|error| JsValue::from_str(&error))?;
            let array = Array::new();
            for action in followups {
                array.push(&encode_storage_action(&action).map_err(|error| {
                    JsValue::from_str(&format!("encode catalog follow-up action: {error}"))
                })?);
            }
            Ok(array)
        }

        #[wasm_bindgen(js_name = completeStorageStep)]
        pub fn complete_storage_step(&mut self, step_id: f64) -> Result<(), JsValue> {
            self.core
                .complete_step(exact_u32(step_id, "catalog storage step id")?)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = isComplete)]
        pub fn is_complete(&self) -> bool {
            self.core.is_complete()
        }

        #[wasm_bindgen(js_name = responseForSmoke)]
        pub fn response_for_smoke(&self) -> Result<JsValue, JsValue> {
            encode_smoke_response(
                self.core
                    .response()
                    .map_err(|error| JsValue::from_str(&error))?,
            )
            .map_err(|error| JsValue::from_str(&error))
        }
    }

    #[wasm_bindgen(js_name = mclone_web_catalog_smoke_execution)]
    pub fn catalog_smoke_execution(
        operation: String,
        options: JsValue,
        active_world_id: String,
    ) -> Result<WebCatalogExecution, JsValue> {
        let id = || -> Result<LocalWorldId, JsValue> {
            LocalWorldId::new(required_string(&options, "id")?)
                .map_err(|error| JsValue::from_str(&error.message))
        };
        let request = match operation.as_str() {
            "listWorlds" => WorldCatalogRequest::ListWorlds,
            "createWorld" => WorldCatalogRequest::CreateWorld {
                options: decode_web_local_world_create_options(&options)
                    .map_err(|error| JsValue::from_str(&error))?,
            },
            "openWorld" => WorldCatalogRequest::OpenWorld { id: id()? },
            "recordWorldPlayed" => WorldCatalogRequest::RecordWorldPlayed { id: id()? },
            "deleteWorld" => WorldCatalogRequest::DeleteWorld { id: id()? },
            "deleteAllLocalWorlds" => WorldCatalogRequest::DeleteAllLocalWorlds {
                include_app_private_content: false,
            },
            "factoryResetLocalData" => WorldCatalogRequest::DeleteAllLocalWorlds {
                include_app_private_content: true,
            },
            _ => {
                return Err(JsValue::from_str(&format!(
                    "unsupported catalog smoke operation {operation:?}"
                )));
            }
        };
        let active_world = if active_world_id.trim().is_empty() {
            None
        } else {
            Some(
                LocalWorldId::new(active_world_id)
                    .map_err(|error| JsValue::from_str(&error.message))?,
            )
        };
        Ok(WebCatalogExecution {
            token: None,
            core: CatalogExecutionCore::new(request, active_world, WEB_WORLD_BACKEND_LABEL)
                .map_err(|error| JsValue::from_str(&error))?,
        })
    }

    fn encode_storage_step(step: StorageStep) -> Result<JsValue, String> {
        let object = Object::new();
        set_number(&object, "stepId", f64::from(step.id))?;
        let transactions = Array::new();
        for transaction in step.transactions {
            let encoded = Object::new();
            let stores = Array::new();
            for store in transaction.stores {
                stores.push(&JsValue::from_str(store.label()));
            }
            set_value(&encoded, "stores", &stores)?;
            set_string(&encoded, "mode", transaction.mode.label())?;
            set_bool(&encoded, "optionalStores", transaction.optional_stores)?;
            let actions = Array::new();
            for action in transaction.actions {
                actions.push(&encode_storage_action(&action)?);
            }
            set_value(&encoded, "actions", &actions)?;
            transactions.push(&encoded);
        }
        set_value(&object, "transactions", &transactions)?;
        Ok(object.into())
    }

    fn encode_storage_action(action: &StorageAction) -> Result<JsValue, String> {
        let object = Object::new();
        set_number(&object, "actionId", f64::from(action.id))?;
        set_string(&object, "store", action.kind.store().label())?;
        match &action.kind {
            StorageActionKind::GetAll {
                needs_timestamp, ..
            } => {
                set_string(&object, "kind", "get-all")?;
                set_bool(&object, "needsTimestamp", *needs_timestamp)?;
            }
            StorageActionKind::Get {
                key,
                needs_timestamp,
                ..
            } => {
                set_string(&object, "kind", "get")?;
                set_string(&object, "key", key)?;
                set_bool(&object, "needsTimestamp", *needs_timestamp)?;
            }
            StorageActionKind::AddCatalogRecord(summary) => {
                set_string(&object, "kind", "add")?;
                set_value(&object, "value", &encode_web_catalog_record(summary)?)?;
            }
            StorageActionKind::PutCatalogRecord(summary) => {
                set_string(&object, "kind", "put")?;
                set_value(&object, "value", &encode_web_catalog_record(summary)?)?;
            }
            StorageActionKind::DeleteKey { key, .. } => {
                set_string(&object, "kind", "delete-key")?;
                set_string(&object, "key", key)?;
            }
            StorageActionKind::DeleteIndexRange { index, key, .. } => {
                set_string(&object, "kind", "delete-index-range")?;
                set_string(&object, "index", index)?;
                set_string(&object, "key", key)?;
            }
            StorageActionKind::Clear { .. } => set_string(&object, "kind", "clear")?,
        }
        Ok(object.into())
    }

    fn encode_smoke_response(response: &WorldCatalogResponse) -> Result<JsValue, String> {
        match response {
            WorldCatalogResponse::WorldList { worlds, .. } => {
                let array = Array::new();
                for world in worlds {
                    array.push(&encode_web_local_world_summary(world)?);
                }
                Ok(array.into())
            }
            WorldCatalogResponse::WorldCreated { summary }
            | WorldCatalogResponse::WorldOpened { summary }
            | WorldCatalogResponse::WorldPlayRecorded { summary } => {
                encode_web_local_world_summary(summary)
            }
            WorldCatalogResponse::WorldDeleted { id } => {
                let object = Object::new();
                set_string(&object, "id", id.as_str())?;
                Ok(object.into())
            }
            WorldCatalogResponse::AllLocalWorldsDeleted { deleted_count } => {
                let object = Object::new();
                set_number(&object, "deletedCount", *deleted_count as f64)?;
                Ok(object.into())
            }
        }
    }

    fn required_string(value: &JsValue, name: &str) -> Result<String, JsValue> {
        Reflect::get(value, &JsValue::from_str(name))
            .map_err(|error| JsValue::from_str(&format!("read catalog smoke field: {error:?}")))?
            .as_string()
            .ok_or_else(|| JsValue::from_str(&format!("catalog smoke field {name} is missing")))
    }

    fn exact_u32(value: f64, label: &str) -> Result<u32, JsValue> {
        if !value.is_finite() || value.fract() != 0.0 || value < 0.0 || value > f64::from(u32::MAX)
        {
            return Err(JsValue::from_str(&format!("{label} must be a u32")));
        }
        Ok(value as u32)
    }

    fn set_value(object: &Object, name: &str, value: &JsValue) -> Result<(), String> {
        Reflect::set(object, &JsValue::from_str(name), value)
            .map(|_| ())
            .map_err(|error| format!("set catalog storage field {name}: {error:?}"))
    }

    fn set_string(object: &Object, name: &str, value: &str) -> Result<(), String> {
        set_value(object, name, &JsValue::from_str(value))
    }

    fn set_number(object: &Object, name: &str, value: f64) -> Result<(), String> {
        set_value(object, name, &JsValue::from_f64(value))
    }

    fn set_bool(object: &Object, name: &str, value: bool) -> Result<(), String> {
        set_value(object, name, &JsValue::from_bool(value))
    }

    pub use self::WebCatalogExecution as ExportedWebCatalogExecution;
}

#[cfg(target_arch = "wasm32")]
pub use wasm::ExportedWebCatalogExecution as WebCatalogExecution;
