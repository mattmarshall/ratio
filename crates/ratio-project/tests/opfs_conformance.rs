//! Local test transport over the production Rust OPFS ObjectStore adapter.
use anyhow::{ensure, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{OpfsProbeOperation, OpfsProbeReply, OpfsProbeRequest};
use ratio_store::{opfs::OpfsStore, ConfigStore, FileBook, Journal, ObjectStore, SeqLog};
use std::sync::{Arc, Mutex};

static OUTPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
const FINAL: &str = "923bede0abad250c1ea53a8b12f2fe8fa7fbc17abf77de476b535545bbe04502";

// A real nested VM trap must escape the host's I/O refusal mapping. This
// export belongs only to the manual conformance module, never the core API.
#[no_mangle]
pub extern "C" fn ratio_opfs_test_trap() {
    core::arch::wasm32::unreachable();
}

#[no_mangle]
pub extern "C" fn ratio_opfs_alloc(length: usize) -> *mut u8 {
    assert!(length <= ratio_store::opfs::MAX_FRAME);
    Box::into_raw(vec![0u8; length].into_boxed_slice()) as *mut u8
}
#[no_mangle]
pub unsafe extern "C" fn ratio_opfs_free(pointer: *mut u8, length: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        pointer, length,
    )));
}
#[no_mangle]
pub unsafe extern "C" fn ratio_opfs_validate(pointer: *const u8, length: usize) -> i32 {
    i32::from(ratio_store::opfs::decode_frame(std::slice::from_raw_parts(pointer, length)).is_ok())
}
#[no_mangle]
pub extern "C" fn ratio_opfs_result_ptr() -> *const u8 {
    OUTPUT.lock().unwrap().as_ptr()
}
#[no_mangle]
pub extern "C" fn ratio_opfs_result_len() -> usize {
    OUTPUT.lock().unwrap().len()
}

fn retained(store: Arc<dyn ObjectStore>) -> Result<(Vec<u8>, String)> {
    let alpha = FileBook::on_store("alpha", store.clone())?;
    let beta = FileBook::on_store("beta", store.clone())?;
    let left = backend_fixture::inspect(&alpha);
    let right = backend_fixture::inspect(&beta);
    ensure!(
        left == right
            && left.0.len() == 3
            && left.2[3] == FINAL
            && left.1.iter().all(|p| p.len() == 2)
            && left.3 == (21, 21),
        "retained fixture bytes, prefixes, planes, or figures differ"
    );
    let drill = FileBook::on_store("append-drill", store)?;
    let entries = drill.entries()?;
    ensure!(
        matches!(entries.len(), 1 | 2)
            && entries[0].id == "before"
            && (entries.len() == 1 || entries[1].id == "after"),
        "write drill lost an entry"
    );
    let shape = backend_fixture::inspect(&drill);
    ensure!(
        shape.3 == if entries.len() == 1 { (7, 7) } else { (14, 14) },
        "write drill figures differ"
    );
    Ok((
        serde_json::to_vec(&left)?,
        format!(
            "drill height {}; digest {}; trial balance {} / {}",
            entries.len(),
            shape.2.last().unwrap(),
            shape.3 .0,
            shape.3 .1
        ),
    ))
}

fn run(request: &OpfsProbeRequest) -> Result<(Vec<u8>, String, bool)> {
    let store = Arc::new(OpfsStore::bind(u32::try_from(request.binding)?)?);
    match OpfsProbeOperation::try_from(request.operation)? {
        OpfsProbeOperation::Seed => {
            ensure!(
                store.list("")?.is_empty(),
                "fixture exists; use replay instead"
            );
            let alpha = backend_fixture::exercise_with(|| {
                FileBook::on_store("alpha", store.clone()).unwrap()
            });
            let beta = backend_fixture::exercise_with(|| {
                FileBook::on_store("beta", store.clone()).unwrap()
            });
            ensure!(alpha == beta && alpha.2[3] == FINAL, "OPFS fixture differs");
            ensure!(
                store.put_if_absent("binary/bytes", b"\0\xff\nexact")?,
                "binary object already present"
            );
            ensure!(
                store.get("binary/bytes")?.as_deref() == Some(b"\0\xff\nexact"),
                "binary bytes differ"
            );
            for key in ["unicode/\u{e000}", "unicode/\u{10000}"] {
                ensure!(
                    store.put_if_absent(key, b"bytes")?,
                    "unicode object already present"
                );
            }
            ensure!(
                store.list("unicode/")? == vec!["unicode/\u{e000}", "unicode/\u{10000}"],
                "UTF-8 key order differs"
            );
            let limit = SeqLog::new(store.clone(), "limit/");
            ensure!(
                limit.claim(u64::MAX, b"last")?,
                "height boundary was not created"
            );
            ensure!(limit.height()? == u64::MAX, "height lost integer precision");
            ensure!(
                limit
                    .append(b"must not wrap")
                    .unwrap_err()
                    .to_string()
                    .contains("does not fit in 64 bits"),
                "full sequence failed for another reason"
            );
            for plane in [
                "journal",
                "deliveries",
                "entities",
                "facts",
                "actions",
                "explanations",
                "closes",
            ] {
                let log = SeqLog::new(store.clone(), format!("claims/{plane}/"));
                ensure!(
                    log.claim(1, b"first")? && !log.claim(1, b"overwrite")?,
                    "conditional claim differs"
                );
                ensure!(
                    log.get(1)?.as_deref() == Some(b"first"),
                    "acknowledged bytes changed"
                );
                let gap = SeqLog::new(store.clone(), format!("holes/{plane}/"));
                ensure!(gap.claim(2, b"later")?, "gap fixture was not created");
                let error = gap.for_each_since(0, &mut |_, _| Ok(())).unwrap_err();
                ensure!(
                    error.to_string().contains("gap"),
                    "missing sequence failed for another reason"
                );
            }
            // The shared fixture's closes plane contains opaque records. Keep
            // it immutable; a separate valid book exercises dated append.
            let mut drill = FileBook::on_store("append-drill", store.clone())?;
            let config = drill.put(b"rules = []\n")?;
            drill.set_active(&config)?;
            drill.append(&backend_fixture::entry("before", &config))?;
            let (bytes, detail) = retained(store)?;
            Ok((bytes, format!("all native fixture bytes/pins/planes/figures, claims, holes, binary, UTF-8, and u64 height checks agree; {detail}"), true))
        }
        OpfsProbeOperation::Replay => {
            let (bytes, detail) = retained(store)?;
            Ok((
                bytes,
                format!("cold replay and book isolation agree; {detail}"),
                false,
            ))
        }
        operation @ (OpfsProbeOperation::Append | OpfsProbeOperation::Interrupt) => {
            let mut book = FileBook::on_store("append-drill", store.clone())?;
            ensure!(
                book.entries()?.len() == 1,
                "fixture already appended; unknown outcomes require reconciliation"
            );
            let config = book
                .active()?
                .ok_or_else(|| anyhow::anyhow!("fixture has no active configuration"))?;
            let outcome = book.append(&backend_fixture::entry("after", &config));
            if operation == OpfsProbeOperation::Interrupt {
                ensure!(
                    outcome.is_err(),
                    "interruption unexpectedly acknowledged a write"
                );
                Ok((
                    Vec::new(),
                    "partial append refused without acknowledgment; reload the owner to recover"
                        .into(),
                    false,
                ))
            } else {
                outcome?;
                let (bytes, detail) = retained(store)?;
                Ok((
                    bytes,
                    format!(
                        "one append acknowledged; both fixture books remain unchanged; {detail}"
                    ),
                    true,
                ))
            }
        }
        _ => {
            anyhow::bail!("operation belongs to the ownership host, not the kernel test transport")
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn ratio_opfs_probe(pointer: *const u8, length: usize) {
    let request = OpfsProbeRequest::decode(std::slice::from_raw_parts(pointer, length));
    let reply = match request {
        Ok(request) => match run(&request) {
            Ok((fixture_json, detail, write_acknowledged)) => OpfsProbeReply {
                correlation_id: request.correlation_id,
                passed: true,
                fixture_json,
                detail,
                write_acknowledged,
                ..Default::default()
            },
            Err(error) => OpfsProbeReply {
                correlation_id: request.correlation_id,
                detail: error.to_string(),
                ..Default::default()
            },
        },
        Err(error) => OpfsProbeReply {
            detail: error.to_string(),
            ..Default::default()
        },
    };
    *OUTPUT.lock().unwrap() = reply.encode_to_vec();
}
