//! OPFS's claim is exclusive worker ownership, not create:true. See
//! OPFSJournal.tla, committed before this adapter. Every host call is bound
//! explicitly; a wasm instance must not invent a global default backend.
use crate::{Digest, ObjectStore};
use anyhow::{bail, ensure, Context, Result};
use prost::Message;
use ratio_proto::ratio::storage::v1::{OpfsKeys, OpfsObject};

pub const MAX_FRAME: usize = 16 * 1024 * 1024;

extern "C" {
    fn ratio_opfs_owner(binding: u32) -> i32;
    fn ratio_opfs_put(binding: u32, frame: *const u8, length: usize) -> i32;
    fn ratio_opfs_get(binding: u32, key: *const u8, length: usize) -> i32;
    fn ratio_opfs_list(binding: u32, prefix: *const u8, length: usize) -> i32;
    fn ratio_opfs_read(binding: u32, output: *mut u8, length: usize) -> i32;
    fn ratio_opfs_height(binding: u32, prefix: *const u8, length: usize, output: *mut u64) -> i32;
}

pub struct OpfsStore {
    binding: u32,
}

fn address(object: &OpfsObject) -> Digest {
    let mut unsigned = object.clone();
    unsigned.digest.clear();
    Digest::of(&unsigned.encode_to_vec())
}

pub fn decode_frame(bytes: &[u8]) -> Result<OpfsObject> {
    ensure!(
        bytes.len() <= MAX_FRAME,
        "OPFS object exceeds the frame limit"
    );
    let object = OpfsObject::decode(bytes).context("invalid OPFS object protobuf")?;
    ensure!(
        object.format_version == 1 && !object.key.is_empty(),
        "unsupported OPFS object"
    );
    ensure!(
        object.encode_to_vec() == bytes,
        "noncanonical OPFS object encoding"
    );
    ensure!(
        Digest::parse(&object.digest)? == address(&object),
        "OPFS object digest mismatch"
    );
    Ok(object)
}

impl OpfsStore {
    pub fn bind(binding: u32) -> Result<Self> {
        ensure!(
            binding != 0 && unsafe { ratio_opfs_owner(binding) } == 1,
            "OPFS backend binding has no exclusive owner"
        );
        Ok(Self { binding })
    }

    fn reply(&self, length: i32) -> Result<Vec<u8>> {
        ensure!(
            length >= 0 && length as usize <= MAX_FRAME,
            "OPFS host read refused"
        );
        let mut bytes = vec![0; length as usize];
        ensure!(
            unsafe { ratio_opfs_read(self.binding, bytes.as_mut_ptr(), bytes.len()) } == length,
            "OPFS host returned a different object length"
        );
        Ok(bytes)
    }
}

impl ObjectStore for OpfsStore {
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        ensure!(!key.is_empty(), "OPFS object key is empty");
        ensure!(
            key.len()
                .checked_add(body.len())
                .is_some_and(|n| n <= MAX_FRAME - 128),
            "OPFS object exceeds the frame limit"
        );
        let mut object = OpfsObject {
            key: key.into(),
            body: body.into(),
            digest: String::new(),
            format_version: 1,
        };
        object.digest = address(&object).as_str().into();
        let bytes = object.encode_to_vec();
        match unsafe { ratio_opfs_put(self.binding, bytes.as_ptr(), bytes.len()) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => bail!("OPFS write refused; the owner may require recovery"),
        }
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let length = unsafe { ratio_opfs_get(self.binding, key.as_ptr(), key.len()) };
        if length == -1 {
            return Ok(None);
        }
        let object = decode_frame(&self.reply(length)?)?;
        ensure!(
            object.key == key,
            "OPFS binding returned another object's bytes"
        );
        Ok(Some(object.body))
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let length = unsafe { ratio_opfs_list(self.binding, prefix.as_ptr(), prefix.len()) };
        let keys = OpfsKeys::decode(self.reply(length)?.as_slice())?.keys;
        ensure!(
            keys.iter().all(|key| key.starts_with(prefix))
                && keys.windows(2).all(|pair| pair[0] < pair[1]),
            "OPFS binding returned foreign, repeated, or unordered keys"
        );
        Ok(keys)
    }

    fn max_sequence(&self, prefix: &str) -> Result<u64> {
        let mut height = 0;
        ensure!(
            unsafe { ratio_opfs_height(self.binding, prefix.as_ptr(), prefix.len(), &mut height) }
                == 1,
            "OPFS height lookup refused"
        );
        Ok(height)
    }
}
