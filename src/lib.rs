#[allow(warnings)]
mod bindings;
use serde_json::Value as JsonValue;

use bindings::{
    exports::supabase::wrappers::routines::Guest,
    supabase::wrappers::{
        http, time,
        types::{Cell, Context, FdwError, FdwResult, OptionsType, Row, TypeOid},
        utils,
    },
};

#[derive(Debug, Default)]
struct ExampleFdw {
    base_url: String,
    src_rows: Vec<JsonValue>,
    src_idx: usize,
}

// pointer for the static FDW instance
static mut INSTANCE: *mut ExampleFdw = std::ptr::null_mut::<ExampleFdw>();

impl ExampleFdw {
    // initialise FDW instance
    fn init_instance() {
        let instance = Self::default();
        unsafe {
            INSTANCE = Box::leak(Box::new(instance));
        }
    }

    fn this_mut() -> &'static mut Self {
        unsafe { &mut (*INSTANCE) }
    }
}

impl Guest for ExampleFdw {
    fn host_version_requirement() -> String {
        // semver expression for Wasm FDW host version requirement
        // ref: https://docs.rs/semver/latest/semver/enum.Op.html
        "^0.1.0".to_string()
    }

    fn init(ctx: &Context) -> FdwResult {
        Self::init_instance();
        let this = Self::this_mut();
    
        // get API URL from foreign server options if it is specified
        let opts = ctx.get_options(OptionsType::Server);
        this.base_url = opts.require_or("base_url", "https://docs.google.com/spreadsheets/d");
    
        Ok(())
    }

    fn begin_scan(ctx: &Context) -> FdwResult {
        let this = Self::this_mut();
    
        // get sheet id from foreign table options and make the request URL
        let opts = ctx.get_options(OptionsType::Table);
        let sheet_id = opts.require("sheet_id")?;
        let url = format!("{}/{}/gviz/tq?tqx=out:json", this.base_url, sheet_id);
    
        // make up request headers
        let headers: Vec<(String, String)> = vec![
            ("user-agent".to_owned(), "Sheets FDW".to_owned()),
            // header to make JSON response more cleaner
            ("x-datasource-auth".to_owned(), "true".to_owned()),
        ];
    
        // make a request to Google API and parse response as JSON
        let req = http::Request {
            method: http::Method::Get,
            url,
            headers,
            body: String::default(),
        };
        let resp = http::get(&req)?;
        // remove invalid prefix from response to make a valid JSON string
        let body = resp.body.strip_prefix(")]}'\n").ok_or("invalid response")?;
        let resp_json: JsonValue = serde_json::from_str(body).map_err(|e| e.to_string())?;
    
        // extract source rows from response
        this.src_rows = resp_json
            .pointer("/table/rows")
            .ok_or("cannot get rows from response")
            .map(|v| v.as_array().unwrap().to_owned())?;
    
        // output a Postgres INFO to user (visible in psql), also useful for debugging
        utils::report_info(&format!(
            "We got response array length: {}",
            this.src_rows.len()
        ));
    
        Ok(())
    }

  fn iter_scan(ctx: &Context, row: &Row) -> Result<Option<u32>, FdwError> {
    let this = Self::this_mut();

    // if all source rows are consumed, stop data scan
    if this.src_idx >= this.src_rows.len() {
        return Ok(None);
    }

    // extract current source row, an example of the source row in JSON:
    // {
    //   "c": [{
    //      "v": 1.0,
    //      "f": "1"
    //    }, {
    //      "v": "Erlich Bachman"
    //    }, null, null, null, null, { "v": null }
    //    ]
    // }
    let src_row = &this.src_rows[this.src_idx];

    // loop through each target column, map source cell to target cell
    for tgt_col in ctx.get_columns() {
        let (tgt_col_num, tgt_col_name) = (tgt_col.num(), tgt_col.name());
        if let Some(src) = src_row.pointer(&format!("/c/{}/v", tgt_col_num - 1)) {
            // we only support I64 and String cell types here, add more type
            // conversions if you need
            let cell = match tgt_col.type_oid() {
                TypeOid::I64 => src.as_f64().map(|v| Cell::I64(v as _)),
                TypeOid::String => src.as_str().map(|v| Cell::String(v.to_owned())),
                _ => {
                    return Err(format!(
                        "column {} data type is not supported",
                        tgt_col_name
                    ));
                }
            };

            // push the cell to target row
            row.push(cell.as_ref());
        } else {
            row.push(None);
        }
    }

    // advance to next source row
    this.src_idx += 1;

    // tell Postgres we've done one row, and need to scan the next row
    Ok(Some(0))
}

    fn re_scan(_ctx: &Context) -> FdwResult {
        Err("re_scan on foreign table is not supported".to_owned())
    }

    fn end_scan(_ctx: &Context) -> FdwResult {
        let this = Self::this_mut();
        this.src_rows.clear();
        Ok(())
    }

    fn begin_modify(_ctx: &Context) -> FdwResult {
        Err("modify on foreign table is not supported".to_owned())
    }

    fn insert(_ctx: &Context, _row: &Row) -> FdwResult {
        Ok(())
    }

    fn update(_ctx: &Context, _rowid: Cell, _row: &Row) -> FdwResult {
        Ok(())
    }

    fn delete(_ctx: &Context, _rowid: Cell) -> FdwResult {
        Ok(())
    }

    fn end_modify(_ctx: &Context) -> FdwResult {
        Ok(())
    }
}

bindings::export!(ExampleFdw with_types_in bindings);

// #[allow(warnings)]
// mod bindings;
// use serde_json::Value as JsonValue;

// use bindings::{
//     exports::supabase::wrappers::routines::Guest,
//     supabase::wrappers::{
//         http, time,
//         types::{Cell, Context, FdwError, FdwResult, OptionsType, Row, TypeOid},
//         utils,
//     },
// };

// #[derive(Debug, Default)]
// struct SquareFdw {
//     base_url: String,
//     access_token: String,
//     object: String,
//     src_rows: Vec<JsonValue>,
//     src_idx: usize,
// }

// // Pointer for the static FDW instance
// static mut INSTANCE: *mut SquareFdw = std::ptr::null_mut::<SquareFdw>();

// impl SquareFdw {
//     // Initialize FDW instance
//     fn init_instance() {
//         let instance = Self::default();
//         unsafe {
//             INSTANCE = Box::leak(Box::new(instance));
//         }
//     }

//     fn this_mut() -> &'static mut Self {
//         unsafe { &mut (*INSTANCE) }
//     }
// }

// impl Guest for SquareFdw {
//     fn host_version_requirement() -> String {
//         // Semver expression for Wasm FDW host version requirement
//         // ref: https://docs.rs/semver/latest/semver/enum.Op.html
//         "^0.1.0".to_string()
//     }

//     fn init(ctx: &Context) -> FdwResult {
//         Self::init_instance();
//         let this = Self::this_mut();

//         // Retrieve server options (e.g., access token and base URL)
//         let server_opts = ctx.get_options(OptionsType::Server);
//         this.base_url = server_opts.require_or("api_url","https://connect.squareup.com/v2")?;
//         this.access_token = server_opts.require("access_token")?;

//         Ok(())
//     }

//     fn begin_scan(ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();

//         // Retrieve table options (e.g., object type)
//         let table_opts = ctx.get_options(OptionsType::Table);
//         this.object = table_opts.require("object")?;

//         let url = match this.object.as_str() {
//             "customers" => format!("{}/customers", this.base_url),
//             "invoices" => format!("{}/invoices", this.base_url),
//             "payments" => format!("{}/payments", this.base_url),
//             "orders" => format!("{}/orders/search", this.base_url),
//             "catalog" => format!("{}/catalog/list", this.base_url),
//             _ => return Err(format!("Unknown object type: {}", this.object)),
//         };

//         let headers = vec![
//             ("Authorization".to_string(), format!("Bearer {}", this.access_token)),
//             ("Content-Type".to_string(), "application/json"),
//             ("Accept".to_string(), "application/json"),
//         ];

//         // For certain endpoints, use POST with an empty body or appropriate parameters
//         let (method, body) = match this.object.as_str() {
//             "orders" => (http::Method::Post, "{\"limit\": 100}".to_string()),
//             "catalog" => (http::Method::Post, "{\"types\": [\"ITEM\"]}".to_string()),
//             _ => (http::Method::Get, String::new()),
//         };

//         let req = http::Request {
//             method,
//             url,
//             headers,
//             body,
//         };
//         let resp = http::request(&req)?;
//         let resp_json: JsonValue = serde_json::from_str(&resp.body).map_err(|e| e.to_string())?;

//         // Extract relevant data based on object type
//         this.src_rows = match this.object.as_str() {
//             "customers" => resp_json["customers"].as_array().cloned().unwrap_or_default(),
//             "invoices" => resp_json["invoices"].as_array().cloned().unwrap_or_default(),
//             "payments" => resp_json["payments"].as_array().cloned().unwrap_or_default(),
//             "orders" => resp_json["orders"].as_array().cloned().unwrap_or_default(),
//             "catalog" => resp_json["objects"].as_array().cloned().unwrap_or_default(),
//             _ => Vec::new(),
//         };

//         this.src_idx = 0;

//         utils::report_info(&format!(
//             "Retrieved {} records for {}",
//             this.src_rows.len(),
//             this.object
//         ));

//         Ok(())
//     }

//     fn iter_scan(ctx: &Context, row: &Row) -> Result<Option<u32>, FdwError> {
//         let this = Self::this_mut();

//         if this.src_idx >= this.src_rows.len() {
//             return Ok(None);
//         }

//         let src_row = &this.src_rows[this.src_idx];
//         let tgt_cols = ctx.get_columns();

//         for tgt_col in tgt_cols {
//             let tgt_col_name = tgt_col.name();
//             let src_value = src_row.get(&tgt_col_name).ok_or_else(|| {
//                 format!("Source column '{}' not found in Square data", tgt_col_name)
//             })?;

//             let cell = match tgt_col.type_oid() {
//                 TypeOid::Bool => src_value.as_bool().map(Cell::Bool),
//                 TypeOid::String => src_value.as_str().map(|v| Cell::String(v.to_owned())),
//                 TypeOid::Int4 => src_value.as_i64().map(|v| Cell::Int4(v as i32)),
//                 TypeOid::Int8 => src_value.as_i64().map(Cell::Int8),
//                 TypeOid::Float8 => src_value.as_f64().map(Cell::Float8),
//                 TypeOid::Json => Some(Cell::Json(src_value.to_string())),
//                 TypeOid::Timestamp => {
//                     if let Some(s) = src_value.as_str() {
//                         let ts = time::parse_from_rfc3339(s)?;
//                         Some(Cell::Timestamp(ts))
//                     } else {
//                         None
//                     }
//                 }
//                 _ => None,
//             };

//             row.push(cell.as_ref());
//         }

//         this.src_idx += 1;

//         Ok(Some(0))
//     }

//     fn re_scan(_ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();
//         this.src_idx = 0;
//         Ok(())
//     }

//     fn end_scan(_ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();
//         this.src_rows.clear();
//         Ok(())
//     }

//     fn begin_modify(ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();

//         let table_opts = ctx.get_options(OptionsType::Table);
//         this.object = table_opts.require("object")?;

//         Ok(())
//     }

//     fn insert(ctx: &Context, row: &Row) -> FdwResult {
//         let this = Self::this_mut();

//         let (url, method) = match this.object.as_str() {
//             "customers" => (format!("{}/customers", this.base_url), http::Method::Post),
//             "invoices" => (format!("{}/invoices", this.base_url), http::Method::Post),
//             "payments" => (format!("{}/payments", this.base_url), http::Method::Post),
//             "orders" => (format!("{}/orders", this.base_url), http::Method::Post),
//             "catalog" => (format!("{}/catalog/object", this.base_url), http::Method::Post),
//             _ => return Err(format!("Insert not supported for object type: {}", this.object)),
//         };

//         let body_json = build_body_json(ctx, row)?;

//         let body = serde_json::to_string(&body_json).map_err(|e| e.to_string())?;

//         let headers = vec![
//             ("Authorization".to_string(), format!("Bearer {}", this.access_token)),
//             ("Content-Type".to_string(), "application/json"),
//             ("Accept".to_string(), "application/json"),
//         ];

//         let req = http::Request {
//             method,
//             url,
//             headers,
//             body,
//         };

//         let resp = http::request(&req)?;
//         if resp.status_code >= 200 && resp.status_code < 300 {
//             Ok(())
//         } else {
//             Err(format!("Failed to insert: {}", resp.body))
//         }
//     }

//     fn update(ctx: &Context, rowid: Cell, row: &Row) -> FdwResult {
//         let this = Self::this_mut();

//         let id = match rowid {
//             Cell::String(s) => s,
//             Cell::Int4(i) => i.to_string(),
//             Cell::Int8(i) => i.to_string(),
//             _ => return Err("Invalid rowid type".to_owned()),
//         };

//         let (url, method) = match this.object.as_str() {
//             "customers" => (format!("{}/customers/{}", this.base_url, id), http::Method::Put),
//             "invoices" => (format!("{}/invoices/{}", this.base_url, id), http::Method::Put),
//             "orders" => (format!("{}/orders/{}", this.base_url, id), http::Method::Put),
//             "catalog" => (format!("{}/catalog/object", this.base_url), http::Method::Put),
//             _ => return Err(format!("Update not supported for object type: {}", this.object)),
//         };

//         let mut body_json = build_body_json(ctx, row)?;
//         // Include 'id' in body for certain objects
//         match this.object.as_str() {
//             "invoices" | "catalog" => {
//                 body_json["id"] = JsonValue::String(id.clone());
//                 // Handle 'version' if required
//             }
//             _ => {}
//         }

//         let body = serde_json::to_string(&body_json).map_err(|e| e.to_string())?;

//         let headers = vec![
//             ("Authorization".to_string(), format!("Bearer {}", this.access_token)),
//             ("Content-Type".to_string(), "application/json"),
//             ("Accept".to_string(), "application/json"),
//         ];

//         let req = http::Request {
//             method,
//             url,
//             headers,
//             body,
//         };

//         let resp = http::request(&req)?;
//         if resp.status_code >= 200 && resp.status_code < 300 {
//             Ok(())
//         } else {
//             Err(format!("Failed to update: {}", resp.body))
//         }
//     }

//     fn delete(_ctx: &Context, rowid: Cell) -> FdwResult {
//         let this = Self::this_mut();

//         let id = match rowid {
//             Cell::String(s) => s,
//             Cell::Int4(i) => i.to_string(),
//             Cell::Int8(i) => i.to_string(),
//             _ => return Err("Invalid rowid type".to_owned()),
//         };

//         let (url, method) = match this.object.as_str() {
//             "customers" => (format!("{}/customers/{}", this.base_url, id), http::Method::Delete),
//             "invoices" => (
//                 format!("{}/invoices/{}/cancel", this.base_url, id),
//                 http::Method::Post,
//             ),
//             "catalog" => (
//                 format!("{}/catalog/object/{}", this.base_url, id),
//                 http::Method::Delete,
//             ),
//             _ => return Err(format!("Delete not supported for object type: {}", this.object)),
//         };

//         let headers = vec![
//             ("Authorization".to_string(), format!("Bearer {}", this.access_token)),
//             ("Content-Type".to_string(), "application/json"),
//             ("Accept".to_string(), "application/json"),
//         ];

//         let req = http::Request {
//             method,
//             url,
//             headers,
//             body: String::new(),
//         };

//         let resp = http::request(&req)?;
//         if resp.status_code >= 200 && resp.status_code < 300 {
//             Ok(())
//         } else {
//             Err(format!("Failed to delete: {}", resp.body))
//         }
//     }

//     fn end_modify(_ctx: &Context) -> FdwResult {
//         Ok(())
//     }
// }

// // Helper function to build JSON body from row data
// fn build_body_json(ctx: &Context, row: &Row) -> Result<serde_json::Value, String> {
//     let tgt_cols = ctx.get_columns();
//     let mut body_json = serde_json::Map::new();

//     for (col, cell) in tgt_cols.iter().zip(row.cells_iter()) {
//         let col_name = col.name();
//         let value = match cell {
//             Cell::Bool(b) => JsonValue::Bool(*b),
//             Cell::String(s) => JsonValue::String(s.clone()),
//             Cell::Int4(i) => JsonValue::Number((*i).into()),
//             Cell::Int8(i) => JsonValue::Number((*i).into()),
//             Cell::Float8(f) => {
//                 serde_json::Number::from_f64(*f).map(JsonValue::Number).unwrap_or(JsonValue::Null)
//             }
//             Cell::Json(s) => serde_json::from_str(s).map_err(|e| e.to_string())?,
//             Cell::Timestamp(ts) => JsonValue::String(ts.to_rfc3339()),
//             _ => JsonValue::Null,
//         };
//         body_json.insert(col_name.to_string(), value);
//     }
//     Ok(JsonValue::Object(body_json))
// }

// bindings::export!(SquareFdw with_types_in bindings);


// #[allow(warnings)]
// mod bindings;
// use serde_json::Value as JsonValue;

// use bindings::{
//     exports::supabase::wrappers::routines::Guest,
//     supabase::wrappers::{
//         http, time,
//         types::{Cell, Context, FdwError, FdwResult, OptionsType, Row, TypeOid},
//         utils,
//     },
// };

// #[derive(Debug, Default)]
// struct SquareFdw {
//     base_url: String,
//     access_token: String, // Store the access token for Square API
//     src_rows: Vec<JsonValue>,
//     src_idx: usize,
// }

// // pointer for the static FDW instance
// static mut INSTANCE: *mut SquareFdw = std::ptr::null_mut::<SquareFdw>();

// impl SquareFdw {
//     // Initialize FDW instance
//     fn init_instance() {
//         let instance = Self::default();
//         unsafe {
//             INSTANCE = Box::leak(Box::new(instance));
//         }
//     }

//     fn this_mut() -> &'static mut Self {
//         unsafe { &mut (*INSTANCE) }
//     }
// }

// impl Guest for SquareFdw {
//     fn host_version_requirement() -> String {
//         "^0.1.0".to_string() // Wasm FDW host version requirement
//     }

//     fn init(ctx: &Context) -> FdwResult {
//         Self::init_instance();
//         let this = Self::this_mut();

//         let opts = ctx.get_options(OptionsType::Server);

//         // Retrieve and store the base URL and access token from options
//         this.base_url = opts.require_or("api_url", "https://connect.squareup.com/v2");
//         this.access_token = opts.require("access_token")?;

//         Ok(())
//     }

//     fn begin_scan(ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();

//         let opts = ctx.get_options(OptionsType::Table);
//         let object = opts.require("object")?;
//         let url = format!("{}/{}", this.base_url, object);

//         let headers: Vec<(String, String)> = vec![
//             ("Authorization".to_owned(), format!("Bearer {}", this.access_token)), // Add access_token to Authorization header
//             ("Content-Type".to_owned(), "application/json".to_owned()), // Set JSON content type
//         ];

//         let req = http::Request {
//             method: http::Method::Get,
//             url,
//             headers,
//             body: String::default(),
//         };
//         let resp = http::get(&req)?;
//         let resp_json: JsonValue = serde_json::from_str(&resp.body).map_err(|e| e.to_string())?;

//         // Ensure that the response is an array
//         this.src_rows = resp_json
//             .as_array()
//             .map(|v| v.to_owned())
//             .expect("response should be a JSON array");

//         utils::report_info(&format!("Received response with array length: {}", this.src_rows.len()));

//         Ok(())
//     }

//     fn iter_scan(ctx: &Context, row: &Row) -> Result<Option<u32>, FdwError> {
//         let this = Self::this_mut();

//         if this.src_idx >= this.src_rows.len() {
//             return Ok(None);
//         }

//         let src_row = &this.src_rows[this.src_idx];
//         for tgt_col in ctx.get_columns() {
//             let tgt_col_name = tgt_col.name();
//             let src = src_row
//                 .as_object()
//                 .and_then(|v| v.get(&tgt_col_name))
//                 .ok_or(format!("source column '{}' not found", tgt_col_name))?;
//             let cell = match tgt_col.type_oid() {
//                 TypeOid::Bool => src.as_bool().map(Cell::Bool),
//                 TypeOid::String => src.as_str().map(|v| Cell::String(v.to_owned())),
//                 TypeOid::Timestamp => {
//                     if let Some(s) = src.as_str() {
//                         let ts = time::parse_from_rfc3339(s)?;
//                         Some(Cell::Timestamp(ts))
//                     } else {
//                         None
//                     }
//                 }
//                 TypeOid::Json => src.as_object().map(|_| Cell::Json(src.to_string())),
//                 _ => {
//                     return Err(format!(
//                         "column {} data type is not supported",
//                         tgt_col_name
//                     ));
//                 }
//             };

//             row.push(cell.as_ref());
//         }

//         this.src_idx += 1;

//         Ok(Some(0))
//     }

//     fn re_scan(_ctx: &Context) -> FdwResult {
//         Err("re_scan on foreign table is not supported".to_owned())
//     }

//     fn end_scan(_ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();
//         this.src_rows.clear();
//         Ok(())
//     }

//     fn begin_modify(_ctx: &Context) -> FdwResult {
//         Err("modify on foreign table is not supported".to_owned())
//     }

//     fn insert(_ctx: &Context, _row: &Row) -> FdwResult {
//         Ok(())
//     }

//     fn update(_ctx: &Context, _rowid: Cell, _row: &Row) -> FdwResult {
//         Ok(())
//     }

//     fn delete(_ctx: &Context, _rowid: Cell) -> FdwResult {
//         Ok(())
//     }

//     fn end_modify(_ctx: &Context) -> FdwResult {
//         Ok(())
//     }
// }

// bindings::export!(SquareFdw with_types_in bindings);


// #[allow(warnings)]
// mod bindings;
// use serde_json::Value as JsonValue;

// use bindings::{
//     exports::supabase::wrappers::routines::Guest,
//     supabase::wrappers::{
//         http, time,
//         types::{Cell, Context, FdwError, FdwResult, OptionsType, Row, TypeOid},
//         utils,
//     },
// };

// #[derive(Debug, Default)]
// struct ExampleFdw {
//     base_url: String,
//     src_rows: Vec<JsonValue>,
//     src_idx: usize,
// }

// // pointer for the static FDW instance
// static mut INSTANCE: *mut ExampleFdw = std::ptr::null_mut::<ExampleFdw>();

// impl ExampleFdw {
//     // initialise FDW instance
//     fn init_instance() {
//         let instance = Self::default();
//         unsafe {
//             INSTANCE = Box::leak(Box::new(instance));
//         }
//     }

//     fn this_mut() -> &'static mut Self {
//         unsafe { &mut (*INSTANCE) }
//     }
// }

// impl Guest for ExampleFdw {
//     fn host_version_requirement() -> String {
//         // semver expression for Wasm FDW host version requirement
//         // ref: https://docs.rs/semver/latest/semver/enum.Op.html
//         "^0.1.0".to_string()
//     }

//     fn init(ctx: &Context) -> FdwResult {
//         Self::init_instance();
//         let this = Self::this_mut();

//         let opts = ctx.get_options(OptionsType::Server);
//         this.base_url = opts.require_or("api_url", "https://api.github.com");

//         Ok(())
//     }

//     fn begin_scan(ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();

//         let opts = ctx.get_options(OptionsType::Table);
//         let object = opts.require("object")?;
//         let url = format!("{}/{}", this.base_url, object);

//         let headers: Vec<(String, String)> =
//             vec![("user-agent".to_owned(), "Example FDW".to_owned())];

//         let req = http::Request {
//             method: http::Method::Get,
//             url,
//             headers,
//             body: String::default(),
//         };
//         let resp = http::get(&req)?;
//         let resp_json: JsonValue = serde_json::from_str(&resp.body).map_err(|e| e.to_string())?;

//         this.src_rows = resp_json
//             .as_array()
//             .map(|v| v.to_owned())
//             .expect("response should be a JSON array");

//         utils::report_info(&format!("We got response array length: {}", this.src_rows.len()));

//         Ok(())
//     }

//     fn iter_scan(ctx: &Context, row: &Row) -> Result<Option<u32>, FdwError> {
//         let this = Self::this_mut();

//         if this.src_idx >= this.src_rows.len() {
//             return Ok(None);
//         }

//         let src_row = &this.src_rows[this.src_idx];
//         for tgt_col in ctx.get_columns() {
//             let tgt_col_name = tgt_col.name();
//             let src = src_row
//                 .as_object()
//                 .and_then(|v| v.get(&tgt_col_name))
//                 .ok_or(format!("source column '{}' not found", tgt_col_name))?;
//             let cell = match tgt_col.type_oid() {
//                 TypeOid::Bool => src.as_bool().map(Cell::Bool),
//                 TypeOid::String => src.as_str().map(|v| Cell::String(v.to_owned())),
//                 TypeOid::Timestamp => {
//                     if let Some(s) = src.as_str() {
//                         let ts = time::parse_from_rfc3339(s)?;
//                         Some(Cell::Timestamp(ts))
//                     } else {
//                         None
//                     }
//                 }
//                 TypeOid::Json => src.as_object().map(|_| Cell::Json(src.to_string())),
//                 _ => {
//                     return Err(format!(
//                         "column {} data type is not supported",
//                         tgt_col_name
//                     ));
//                 }
//             };

//             row.push(cell.as_ref());
//         }

//         this.src_idx += 1;

//         Ok(Some(0))
//     }

//     fn re_scan(_ctx: &Context) -> FdwResult {
//         Err("re_scan on foreign table is not supported".to_owned())
//     }

//     fn end_scan(_ctx: &Context) -> FdwResult {
//         let this = Self::this_mut();
//         this.src_rows.clear();
//         Ok(())
//     }

//     fn begin_modify(_ctx: &Context) -> FdwResult {
//         Err("modify on foreign table is not supported".to_owned())
//     }

//     fn insert(_ctx: &Context, _row: &Row) -> FdwResult {
//         Ok(())
//     }

//     fn update(_ctx: &Context, _rowid: Cell, _row: &Row) -> FdwResult {
//         Ok(())
//     }

//     fn delete(_ctx: &Context, _rowid: Cell) -> FdwResult {
//         Ok(())
//     }

//     fn end_modify(_ctx: &Context) -> FdwResult {
//         Ok(())
//     }
// }

// bindings::export!(ExampleFdw with_types_in bindings);
