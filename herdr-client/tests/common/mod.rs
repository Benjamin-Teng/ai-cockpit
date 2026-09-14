//! `tests/contract.rs` 與 `tests/events.rs` 共用的 fixture／schema 存取工具。
//!
//! fix round 1 / finding 3：events.rs 新增的「手寫最小案例先過 schema 驗證，再解析成
//! payload 逐欄斷言」需要跟 contract.rs 一樣的 `jsonschema` validator（design D8），抽到
//! 這裡讓兩邊共用，不重造第二份。`tests/common/mod.rs` 是 Rust 慣例：`common/mod.rs`
//! 不會被 cargo 當成獨立的測試二進位檔，只能用 `mod common;` 從其他 `tests/*.rs` 引入。
//!
//! 每個 `tests/*.rs` 各自編譯成獨立的二進位檔，`mod common;` 等於把這個檔案各複製一份
//! 編進去；哪個二進位檔沒用到的 `pub fn` 就會在那個二進位檔觸發 `dead_code`
//! （即使另一個 `tests/*.rs` 有用到）。整份 `allow(dead_code)`，不用逐一標記。
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn load_json(name: &str) -> serde_json::Value {
    let raw = std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name} is not valid JSON: {e}"))
}

pub fn read_lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("failed to read fixture {name}: {e}"))
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// design D8：複製整份 schema 文件，在頂層插入 `$ref` 指到 `schemas.<root>`，整份編譯。
pub fn validator_for_root(document: &serde_json::Value, root: &str) -> jsonschema::Validator {
    let mut rooted = document.clone();
    rooted
        .as_object_mut()
        .expect("schema document must be a JSON object")
        .insert(
            "$ref".to_string(),
            serde_json::Value::String(format!("#/schemas/{root}")),
        );
    jsonschema::draft202012::new(&rooted)
        .unwrap_or_else(|e| panic!("failed to compile \"{root}\" validator: {e}"))
}

pub fn assert_valid(validator: &jsonschema::Validator, instance: &serde_json::Value, label: &str) {
    if let Err(err) = validator.validate(instance) {
        panic!(
            "{label}: instance does not satisfy schema at {}: {err}\ninstance = {instance}",
            err.instance_path()
        );
    }
}

pub struct Schemas {
    pub p22: serde_json::Value,
    pub p20: serde_json::Value,
}

pub fn schemas() -> Schemas {
    Schemas {
        p22: load_json("schema-p22.json"),
        p20: load_json("schema-p20.json"),
    }
}

/// 對兩份 schema 的 `event` 根都驗證通過。events.rs 手寫的最小案例先過這關，再解析成
/// payload 逐欄斷言（fix round 1 / finding 3）。
pub fn assert_valid_as_event(instance: &serde_json::Value, label: &str) {
    let schemas = schemas();
    assert_valid(
        &validator_for_root(&schemas.p22, "event"),
        instance,
        &format!("{label} (protocol 22 event)"),
    );
    assert_valid(
        &validator_for_root(&schemas.p20, "event"),
        instance,
        &format!("{label} (protocol 20 event)"),
    );
}

/// 對兩份 schema 的 `subscription_event` 根都驗證通過（每 pane 訂閱推送的點號事件，例如
/// `pane.agent_status_changed`；fix round 2 / finding 3）。
pub fn assert_valid_as_subscription_event(instance: &serde_json::Value, label: &str) {
    let schemas = schemas();
    assert_valid(
        &validator_for_root(&schemas.p22, "subscription_event"),
        instance,
        &format!("{label} (protocol 22 subscription_event)"),
    );
    assert_valid(
        &validator_for_root(&schemas.p20, "subscription_event"),
        instance,
        &format!("{label} (protocol 20 subscription_event)"),
    );
}

/// fix round 2 / finding 3、fix round 3 再查證：把 `parsed` 重新序列化成 JSON，遞迴比對
/// 它每一個 key 與 `raw` 同名 key 的值——只走 `parsed` 有的 key，所以天生只檢查「已建模
/// 欄位」，不要求 `raw` 沒被建模的欄位也對得上。
///
/// **`allowed_absent`（fix round 3）**：`raw` 缺席某個 key 時，只有這個 key 的裸名稱在
/// `allowed_absent` 裡才放行（跳過比對），否則直接失敗。round 2 的版本是「`raw` 缺席就一律
/// 放行」，Codex 的 round 2 re-review 指出這樣會蓋掉真正的映射錯誤：如果某個欄位的
/// `#[serde(rename = "...")]` 打錯字（例如把 `display_agent` 錯打成 `display_agnet`），
/// 反序列化會因為找不到錯字那個 key 而用 `#[serde(default)]` 補一個空值，重新序列化出來的
/// 也是那個錯字 key；`raw` 當然沒有這個錯字 key，round 2 的規則會直接放行、測試照樣全綠。
/// 現在改成「預設失敗，只有呼叫端明確列出的欄位名稱才放行」：呼叫端寫的是「這個型別真正
/// 選填的欄位叫什麼名字」，跟 `parsed` 實際序列化出來的 key 名稱是兩個獨立來源——如果
/// rename 打錯字，`parsed` 序列化出來的錯字 key 不會出現在呼叫端寫的（正確）allow-list
/// 裡，於是被當成未知的缺席鍵而失敗（見
/// `assert_modeled_fields_match_catches_wrong_serde_rename` 的負向測試）。
///
/// `allowed_absent` 用裸欄位名稱比對、不分巢狀深度或陣列位置（例如 `"label"` 同時允許
/// `PaneInfo.label` 與任何巢狀 pane 陣列元素的 `label` 缺席）——對欄位全部必填的
/// `WorkspaceInfo`／`TabInfo` 傳空陣列，這兩個型別不論在哪個巢狀位置出現，任何欄位缺席都
/// 會被視為錯誤而失敗，仍然有偵測力；有名稱重疊風險的地方（例如 `PaneInfo.label` 選填、
/// 但另一個型別剛好也有必填的同名欄位）由型別各自獨立的窄範圍測試
/// （`workspace_info_maps_all_fields_from_fixture_*` 等，`allowed_absent` 傳空陣列）
/// 補強精確度。
pub fn assert_modeled_fields_match(
    raw: &serde_json::Value,
    parsed: &impl serde::Serialize,
    allowed_absent: &[&str],
    label: &str,
) {
    let parsed_value = serde_json::to_value(parsed).expect("parsed value must serialize to JSON");
    assert_fields_match(raw, &parsed_value, allowed_absent, label);
}

fn assert_fields_match(
    raw: &serde_json::Value,
    parsed: &serde_json::Value,
    allowed_absent: &[&str],
    path: &str,
) {
    match parsed {
        serde_json::Value::Object(map) => {
            let raw_obj = raw
                .as_object()
                .unwrap_or_else(|| panic!("{path}: raw is not an object (parsed is): {raw}"));
            for (key, parsed_val) in map {
                let child_path = format!("{path}.{key}");
                match raw_obj.get(key) {
                    Some(raw_val) => {
                        assert_fields_match(raw_val, parsed_val, allowed_absent, &child_path);
                    }
                    None => assert!(
                        allowed_absent.contains(&key.as_str()),
                        "{child_path}: raw has no key {key:?} and {key:?} is not in allowed_absent — likely a wrong serde rename or a missing field mapping (parsed value = {parsed_val}); if this field is genuinely optional and legitimately absent here, add {key:?} to allowed_absent"
                    ),
                }
            }
        }
        serde_json::Value::Array(parsed_arr) => {
            let raw_arr = raw
                .as_array()
                .unwrap_or_else(|| panic!("{path}: raw is not an array (parsed is): {raw}"));
            assert_eq!(
                parsed_arr.len(),
                raw_arr.len(),
                "{path}: array length mismatch (parsed {} vs raw {})",
                parsed_arr.len(),
                raw_arr.len()
            );
            for (i, (p, r)) in parsed_arr.iter().zip(raw_arr.iter()).enumerate() {
                assert_fields_match(r, p, allowed_absent, &format!("{path}[{i}]"));
            }
        }
        scalar_or_null => {
            assert_eq!(
                raw, scalar_or_null,
                "{path}: value mismatch (parsed {scalar_or_null}, raw {raw})"
            );
        }
    }
}

/// fix round 2 / finding 2、fix round 3 再查證：讀 `schemas.<root>.$defs.<defs_name>.required`
/// 陣列。`schema_file` 是 fix round 3 加的參數——round 2 的版本寫死讀 `schema-p22.json`，
/// 五個表格化測試只依 p22 判定 required/optional，沒有斷言守住「p20 跟 p22 一樣」這個前提；
/// 現在讓呼叫端指定要讀哪一份，`tests/types.rs` 的表格化測試對 `schema-p22.json`、
/// `schema-p20.json` 各跑一次，另加一個測試直接斷言兩份的 `required` 陣列相等（drift 時
/// 印出差異、明確失敗）。
pub fn schema_required_fields(schema_file: &str, root: &str, defs_name: &str) -> Vec<String> {
    let doc = load_json(schema_file);
    doc.pointer(&format!("/schemas/{root}/$defs/{defs_name}/required"))
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("no required array at schemas.{root}.$defs.{defs_name} in {schema_file}")
        })
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| {
                    panic!("non-string entry in {defs_name}.required ({schema_file})")
                })
                .to_string()
        })
        .collect()
}

/// fix round 2 / finding 2（2b／2c）：對 `T` 做表格化的必填欄位測試——「已建模欄位」由
/// `T::default()` 序列化後的 JSON key 集合自動取得（不是手寫清單，不會與 struct 定義脫節），
/// 「schema 必填欄位」由 `schema_required_fields` 讀 `schema_file` 的 `required` 陣列取得，
/// 兩者交集就是「已建模且必填」的欄位：從 `valid_instance` 逐一刪除這些欄位，斷言解析失敗；
/// 差集（已建模但非必填）逐一刪除，斷言解析仍成功（design D13：只對「已建模」的必填欄位
/// 保證偵測缺席，不為了偵測而多建模 schema 有但我們用不到的欄位）。
pub fn assert_required_fields_detected<T>(
    schema_file: &str,
    root: &str,
    defs_name: &str,
    valid_instance: serde_json::Value,
) where
    T: Default + serde::Serialize + serde::de::DeserializeOwned,
{
    let required = schema_required_fields(schema_file, root, defs_name);
    assert_required_fields_detected_against::<T>(
        &required,
        &format!("{defs_name} ({schema_file})"),
        valid_instance,
    );
}

/// 全分支最終 review finding 1：`schemas.<root>.$defs.<defs_name>.oneOf[*]` 底下每個變體用
/// `properties.type.const` 當判別式（例如 `schemas.event.$defs.EventData`）——不像
/// `success_response`／`error_response` 底下那些具名 `$defs`（`schema_required_fields` 直接
/// 讀 `$defs.<defs_name>.required` 就夠），這裡要先在 `oneOf` 陣列裡找到 `type` 等於
/// `type_const` 的那個變體，才能讀它的 `required` 陣列。
pub fn schema_required_fields_for_event_variant(
    schema_file: &str,
    root: &str,
    defs_name: &str,
    type_const: &str,
) -> Vec<String> {
    let doc = load_json(schema_file);
    let variants = doc
        .pointer(&format!("/schemas/{root}/$defs/{defs_name}/oneOf"))
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("no oneOf array at schemas.{root}.$defs.{defs_name} in {schema_file}")
        });
    let variant = variants
        .iter()
        .find(|v| {
            v.pointer("/properties/type/const")
                .and_then(serde_json::Value::as_str)
                == Some(type_const)
        })
        .unwrap_or_else(|| {
            panic!(
                "no oneOf variant with properties.type.const == {type_const:?} in schemas.{root}.$defs.{defs_name} ({schema_file})"
            )
        });
    variant
        .pointer("/required")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| {
            panic!("variant {type_const:?} has no required array (schemas.{root}.$defs.{defs_name}, {schema_file})")
        })
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| {
                    panic!("non-string entry in {type_const:?} required array ({schema_file})")
                })
                .to_string()
        })
        .collect()
}

/// 全分支最終 review finding 1：跟 `assert_required_fields_detected` 同一套表格化判定，但
/// `required` 陣列改用 `schema_required_fields_for_event_variant` 找（`event` root 的
/// `EventData` oneOf 變體用 `type_const` 判別，見上）。
pub fn assert_required_fields_detected_for_event_variant<T>(
    schema_file: &str,
    root: &str,
    defs_name: &str,
    type_const: &str,
    valid_instance: serde_json::Value,
) where
    T: Default + serde::Serialize + serde::de::DeserializeOwned,
{
    let required =
        schema_required_fields_for_event_variant(schema_file, root, defs_name, type_const);
    assert_required_fields_detected_against::<T>(
        &required,
        &format!("{type_const} in {defs_name} ({schema_file})"),
        valid_instance,
    );
}

/// `assert_required_fields_detected`／`assert_required_fields_detected_for_event_variant`
/// 共用的核心判定：「已建模欄位」由 `T::default()` 序列化後的 JSON key 集合自動取得，跟
/// `required`（呼叫端已經算好的 schema 必填欄位清單）取交集就是「已建模且必填」的欄位：從
/// `valid_instance` 逐一刪除這些欄位，斷言解析失敗；差集（已建模但非必填）逐一刪除，斷言
/// 解析仍成功。
fn assert_required_fields_detected_against<T>(
    required: &[String],
    label: &str,
    valid_instance: serde_json::Value,
) where
    T: Default + serde::Serialize + serde::de::DeserializeOwned,
{
    let modeled: Vec<String> = serde_json::to_value(T::default())
        .expect("T::default() must serialize to JSON")
        .as_object()
        .unwrap_or_else(|| panic!("{label}: T::default() did not serialize to an object"))
        .keys()
        .cloned()
        .collect();
    let required_set: std::collections::HashSet<&str> =
        required.iter().map(String::as_str).collect();

    let required_modeled: Vec<&String> = modeled
        .iter()
        .filter(|f| required_set.contains(f.as_str()))
        .collect();
    let optional_modeled: Vec<&String> = modeled
        .iter()
        .filter(|f| !required_set.contains(f.as_str()))
        .collect();
    assert!(
        !required_modeled.is_empty(),
        "{label}: no modeled field is also schema-required"
    );

    for field in &required_modeled {
        let mut instance = valid_instance.clone();
        instance
            .as_object_mut()
            .expect("valid_instance must be a JSON object")
            .remove(field.as_str());
        let result: Result<T, _> = serde_json::from_value(instance);
        assert!(
            result.is_err(),
            "{label}: removing modeled+required field {field:?} must fail to parse, but it parsed successfully"
        );
    }

    for field in &optional_modeled {
        let mut instance = valid_instance.clone();
        instance
            .as_object_mut()
            .expect("valid_instance must be a JSON object")
            .remove(field.as_str());
        let result: Result<T, _> = serde_json::from_value(instance);
        assert!(
            result.is_ok(),
            "{label}: removing modeled-but-optional field {field:?} must still parse, got {:?}",
            result.err()
        );
    }
}
