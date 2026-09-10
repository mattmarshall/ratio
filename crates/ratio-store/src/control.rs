//! Durable post-create control state.
//!
//! The immutable bootstrap is revision zero. Every later configuration or
//! membership decision conditionally claims one exact successor slot.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use prost::Message;
pub use ratio_proto::ratio::storage::v1::{
    control_operation, membership_revision, ConfigPromotion, ControlOperation, ControlReceipt,
    ControlTransition, MembershipRevision,
};

use crate::bootstrap::{valid_book_id, BookBootstrap, BootstrapStore};
use crate::{Digest, ObjectStore};

const TRANSITIONS: &str = "_control/transitions/";
const CONFIG_BLOBS: &str = "_control/config-blobs/";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Principal {
    AuthKitSubject(String),
    Organization(String),
}

#[derive(Clone, Debug)]
pub struct ControlState {
    pub bootstrap_digest: String,
    pub revision: i64,
    pub predecessor_digest: String,
    pub active: Digest,
    pub history: Vec<Digest>,
    pub members: BTreeSet<Principal>,
    explicit_subjects: BTreeSet<String>,
    operations: BTreeMap<String, (String, ControlReceipt)>,
}

impl ControlState {
    pub fn allows(&self, subject: &str, organization: &str, connect: bool) -> bool {
        if connect {
            return self.explicit_subjects.contains(subject);
        }
        self.members
            .contains(&Principal::AuthKitSubject(subject.to_string()))
            || (!organization.is_empty()
                && self
                    .members
                    .contains(&Principal::Organization(organization.to_string())))
    }
}

#[derive(Clone)]
pub struct ControlStore {
    objects: Arc<dyn ObjectStore>,
}

impl ControlStore {
    pub fn new(objects: Arc<dyn ObjectStore>) -> Self {
        Self { objects }
    }

    fn transition_prefix(book_id: &str) -> String {
        format!("{TRANSITIONS}{book_id}/")
    }

    fn transition_key(book_id: &str, revision: i64) -> String {
        format!("{}{revision:020}", Self::transition_prefix(book_id))
    }

    fn config_key(digest: &Digest) -> String {
        format!("{CONFIG_BLOBS}{}", digest.as_str())
    }

    fn canonical<M: Message + Default>(bytes: &[u8], what: &str) -> Result<M> {
        let value = M::decode(bytes).with_context(|| format!("invalid {what} protobuf"))?;
        ensure!(
            value.encode_to_vec() == bytes,
            "{what} protobuf is not canonical"
        );
        Ok(value)
    }

    fn validate_identity(value: &str, what: &str) -> Result<()> {
        ensure!(
            !value.trim().is_empty()
                && value == value.trim()
                && !value.chars().any(char::is_control),
            "{what} is invalid"
        );
        Ok(())
    }

    fn principal(change: &MembershipRevision) -> Result<Principal> {
        use membership_revision::Principal as Wire;
        Self::validate_identity(
            match change.principal.as_ref() {
                Some(Wire::AuthkitSubject(value)) | Some(Wire::OrganizationId(value)) => value,
                None => bail!("membership principal is absent"),
            },
            "membership principal",
        )?;
        Ok(match change.principal.as_ref().expect("checked") {
            Wire::AuthkitSubject(value) => Principal::AuthKitSubject(value.clone()),
            Wire::OrganizationId(value) => Principal::Organization(value.clone()),
        })
    }

    fn operation_bytes(operation: &ControlOperation) -> Result<Vec<u8>> {
        ensure!(
            operation.format_version == 1,
            "unsupported control operation version"
        );
        ensure!(
            operation.expected_revision >= 0,
            "control operation expected revision is negative"
        );
        ensure!(
            valid_book_id(&operation.book_id),
            "invalid control operation book ID"
        );
        Digest::parse(&operation.bootstrap_digest)
            .context("invalid control operation bootstrap digest")?;
        Digest::parse(&operation.expected_predecessor_digest)
            .context("invalid control operation predecessor digest")?;
        Self::validate_identity(&operation.operation_id, "operation ID")?;
        Self::validate_identity(&operation.actor_subject, "control actor")?;
        Self::validate_identity(&operation.actor_provenance, "control actor provenance")?;
        match operation.change.as_ref() {
            Some(control_operation::Change::ConfigPromotion(promotion)) => {
                Digest::parse(&promotion.config_digest)
                    .context("invalid promoted configuration digest")?;
            }
            Some(control_operation::Change::MembershipRevision(change)) => {
                ensure!(
                    membership_revision::Action::try_from(change.action)
                        .ok()
                        .is_some_and(
                            |action| action != membership_revision::Action::Unspecified
                        ),
                    "membership action is unsupported"
                );
                Self::principal(change)?;
            }
            None => bail!("control operation change is absent"),
        }
        Ok(operation.encode_to_vec())
    }

    fn verified_config(&self, digest: &Digest) -> Result<Vec<u8>> {
        let bytes = self
            .objects
            .get(&Self::config_key(digest))?
            .with_context(|| format!("durable configuration {} is missing", digest.short()))?;
        ensure!(
            Digest::of(&bytes) == *digest,
            "durable configuration {} digest mismatch",
            digest.short()
        );
        Ok(bytes)
    }

    pub fn stage_config(&self, bytes: &[u8]) -> Result<Digest> {
        let digest = Digest::of(bytes);
        self.objects
            .put_if_absent(&Self::config_key(&digest), bytes)?;
        ensure!(
            self.verified_config(&digest)? == bytes,
            "staged configuration bytes differ at their digest"
        );
        Ok(digest)
    }

    pub fn config(&self, digest: &Digest) -> Result<Vec<u8>> {
        self.verified_config(digest)
    }

    pub fn read(&self, book_id: &str) -> Result<ControlState> {
        let (publication, bootstrap) = BootstrapStore::new(self.objects.clone())
            .get(book_id)?
            .context("durable book publication is absent")?;
        let active = Digest::parse(&bootstrap.config_digest)?;
        let mut state = ControlState {
            bootstrap_digest: publication.bootstrap_digest.clone(),
            revision: 0,
            predecessor_digest: publication.bootstrap_digest,
            active: active.clone(),
            history: vec![active],
            members: BTreeSet::from([Principal::AuthKitSubject(
                bootstrap
                    .creator_grant
                    .as_ref()
                    .context("bootstrap creator grant is absent")?
                    .subject
                    .clone(),
            )]),
            explicit_subjects: BTreeSet::new(),
            operations: BTreeMap::new(),
        };
        let prefix = Self::transition_prefix(book_id);
        let keys = self.objects.list(&prefix)?;
        for (offset, key) in keys.iter().enumerate() {
            let revision = i64::try_from(offset + 1).context("control revision overflow")?;
            ensure!(
                key == &Self::transition_key(book_id, revision),
                "control transition stream has a gap or invalid key at revision {revision}"
            );
            let bytes = self
                .objects
                .get(key)?
                .with_context(|| format!("control transition {revision} is missing"))?;
            let transition: ControlTransition = Self::canonical(&bytes, "control transition")?;
            ensure!(
                transition.format_version == 1,
                "unsupported control transition version"
            );
            ensure!(
                transition.revision > 0 && transition.revision == revision,
                "control transition revision mismatch"
            );
            let operation = transition
                .operation
                .as_ref()
                .context("control transition operation is absent")?;
            let operation_bytes = Self::operation_bytes(operation)?;
            let operation_digest = Digest::of(&operation_bytes);
            ensure!(
                operation_digest.as_str() == transition.operation_digest,
                "control operation digest mismatch"
            );
            ensure!(
                operation.book_id == book_id,
                "control transition book mismatch"
            );
            ensure!(
                operation.bootstrap_digest == state.bootstrap_digest,
                "control transition bootstrap mismatch"
            );
            ensure!(
                operation.expected_revision == state.revision,
                "control transition expected revision mismatch"
            );
            ensure!(
                operation.expected_predecessor_digest == state.predecessor_digest,
                "control transition predecessor mismatch"
            );
            ensure!(
                !state.operations.contains_key(&operation.operation_id),
                "control operation ID is reused"
            );
            match operation.change.as_ref().expect("validated") {
                control_operation::Change::ConfigPromotion(promotion) => {
                    let digest = Digest::parse(&promotion.config_digest)?;
                    self.verified_config(&digest)?;
                    state.active = digest.clone();
                    state.history.insert(0, digest);
                }
                control_operation::Change::MembershipRevision(change) => {
                    let principal = Self::principal(change)?;
                    match membership_revision::Action::try_from(change.action)? {
                        membership_revision::Action::Grant => {
                            if let Principal::AuthKitSubject(subject) = &principal {
                                state.explicit_subjects.insert(subject.clone());
                            }
                            state.members.insert(principal);
                        }
                        membership_revision::Action::Revoke => {
                            if let Principal::AuthKitSubject(subject) = &principal {
                                state.explicit_subjects.remove(subject);
                            }
                            state.members.remove(&principal);
                        }
                        membership_revision::Action::Unspecified => {
                            bail!("membership action is unsupported")
                        }
                    }
                }
            }
            let transition_digest = Digest::of(&bytes).as_str().to_string();
            let receipt = ControlReceipt {
                book_id: book_id.to_string(),
                bootstrap_digest: state.bootstrap_digest.clone(),
                operation_id: operation.operation_id.clone(),
                operation_digest: operation_digest.as_str().to_string(),
                committed_revision: revision,
                transition_digest: transition_digest.clone(),
            };
            state.operations.insert(
                operation.operation_id.clone(),
                (operation_digest.as_str().to_string(), receipt),
            );
            state.revision = revision;
            state.predecessor_digest = transition_digest;
        }
        // The opening configuration is inside the verified bootstrap, while
        // promoted configurations live at their own durable addresses.
        if state.active != Digest::parse(&bootstrap.config_digest)? {
            self.verified_config(&state.active)?;
        }
        Ok(state)
    }

    pub fn commit(&self, operation: &ControlOperation) -> Result<ControlReceipt> {
        let operation_bytes = Self::operation_bytes(operation)?;
        let operation_digest = Digest::of(&operation_bytes);
        let state = self.read(&operation.book_id)?;
        ensure!(
            state.allows(&operation.actor_subject, "", false),
            "control actor is not a current book member"
        );
        if let Some((existing_digest, receipt)) = state.operations.get(&operation.operation_id) {
            ensure!(
                existing_digest == operation_digest.as_str(),
                "operation ID was already committed with different bytes"
            );
            return Ok(receipt.clone());
        }
        ensure!(
            operation.bootstrap_digest == state.bootstrap_digest,
            "control operation bootstrap mismatch"
        );
        ensure!(
            operation.expected_revision == state.revision
                && operation.expected_predecessor_digest == state.predecessor_digest,
            "control predecessor conflict; reread and review before retrying"
        );
        if let Some(control_operation::Change::ConfigPromotion(promotion)) =
            operation.change.as_ref()
        {
            self.verified_config(&Digest::parse(&promotion.config_digest)?)?;
        }
        let revision = operation
            .expected_revision
            .checked_add(1)
            .context("control revision overflow")?;
        let transition = ControlTransition {
            format_version: 1,
            revision,
            operation: Some(operation.clone()),
            operation_digest: operation_digest.as_str().to_string(),
        };
        let bytes = transition.encode_to_vec();
        if !self
            .objects
            .put_if_absent(&Self::transition_key(&operation.book_id, revision), &bytes)?
        {
            // Two exact retries can pass the same predecessor check before
            // either conditional claim completes. The loser rereads the
            // authority: identical bytes receive the winner's receipt, while
            // any other winner remains a conflict and is never rebased.
            let committed = self.read(&operation.book_id)?;
            if let Some((existing_digest, receipt)) =
                committed.operations.get(&operation.operation_id)
            {
                ensure!(
                    existing_digest == operation_digest.as_str(),
                    "operation ID was already committed with different bytes"
                );
                return Ok(receipt.clone());
            }
            bail!("control predecessor conflict; reread and review before retrying");
        }
        ensure!(
            self.objects
                .get(&Self::transition_key(&operation.book_id, revision))?
                .as_deref()
                == Some(bytes.as_slice()),
            "committed control transition could not be verified"
        );
        Ok(ControlReceipt {
            book_id: operation.book_id.clone(),
            bootstrap_digest: operation.bootstrap_digest.clone(),
            operation_id: operation.operation_id.clone(),
            operation_digest: operation_digest.as_str().to_string(),
            committed_revision: revision,
            transition_digest: Digest::of(&bytes).as_str().to_string(),
        })
    }

    pub fn opening_config(&self, bootstrap: &BookBootstrap, digest: &Digest) -> Option<Vec<u8>> {
        (bootstrap.config_digest == digest.as_str()).then(|| bootstrap.config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::{BookBootstrap, CreatorGrant};
    use crate::{Account, AccountTypeRecord, MemoryStore};
    use std::sync::{Barrier, Mutex};

    fn bootstrap(id: &str, creator: &str) -> BookBootstrap {
        let chart = serde_json::to_vec(&vec![Account {
            dim: 1,
            display_name: "Cash".into(),
            account_type: AccountTypeRecord::Asset,
        }])
        .unwrap();
        let config = b"rules = []\n".to_vec();
        let digest = Digest::of(&config).as_str().to_string();
        BookBootstrap {
            format_version: 1,
            book_id: id.into(),
            kind: 1,
            display_name: id.into(),
            chart_digest: Digest::of(&chart).as_str().into(),
            chart,
            config,
            config_digest: digest.clone(),
            active: digest.clone(),
            history: vec![digest],
            creator_grant: Some(CreatorGrant {
                book_id: id.into(),
                subject: creator.into(),
            }),
            creator_subject: creator.into(),
        }
    }

    fn publish(store: Arc<dyn ObjectStore>, id: &str) -> ControlStore {
        BootstrapStore::new(store.clone())
            .publish(&bootstrap(id, "creator"))
            .unwrap();
        ControlStore::new(store)
    }

    fn operation(
        control: &ControlStore,
        book: &str,
        id: &str,
        actor: &str,
        change: control_operation::Change,
    ) -> ControlOperation {
        let state = control.read(book).unwrap();
        ControlOperation {
            format_version: 1,
            book_id: book.into(),
            bootstrap_digest: state.bootstrap_digest,
            expected_revision: state.revision,
            expected_predecessor_digest: state.predecessor_digest,
            operation_id: id.into(),
            actor_subject: actor.into(),
            actor_provenance: "test-authorizer".into(),
            change: Some(change),
        }
    }

    fn promote(digest: &Digest) -> control_operation::Change {
        control_operation::Change::ConfigPromotion(ConfigPromotion {
            config_digest: digest.as_str().into(),
        })
    }

    fn member(action: membership_revision::Action, subject: &str) -> control_operation::Change {
        control_operation::Change::MembershipRevision(MembershipRevision {
            action: action as i32,
            principal: Some(membership_revision::Principal::AuthkitSubject(
                subject.into(),
            )),
        })
    }

    #[test]
    fn a_fresh_reader_recovers_two_promotions_and_grant_revoke_history() {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let control = publish(objects.clone(), "book");
        let first = control.stage_config(b"first exact bytes\n").unwrap();
        let second = control.stage_config(b"second exact bytes\n").unwrap();
        assert_ne!(
            control.read("book").unwrap().active,
            first,
            "saving bytes alone must not promote them"
        );
        control
            .commit(&operation(
                &control,
                "book",
                "p1",
                "creator",
                promote(&first),
            ))
            .unwrap();
        control
            .commit(&operation(
                &control,
                "book",
                "grant",
                "creator",
                member(membership_revision::Action::Grant, "guest"),
            ))
            .unwrap();
        control
            .commit(&operation(
                &control,
                "book",
                "p2",
                "guest",
                promote(&second),
            ))
            .unwrap();
        control
            .commit(&operation(
                &control,
                "book",
                "revoke",
                "creator",
                member(membership_revision::Action::Revoke, "guest"),
            ))
            .unwrap();

        let recovered = ControlStore::new(objects).read("book").unwrap();
        assert_eq!(recovered.revision, 4);
        assert_eq!(recovered.active, second);
        assert_eq!(recovered.history[0], second);
        assert_eq!(recovered.history[1], first);
        assert!(!recovered.allows("guest", "", false));
        assert!(recovered.allows("creator", "", false));
    }

    #[test]
    fn exact_retries_return_the_original_receipt_and_reuse_with_other_bytes_refuses() {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let control = publish(objects, "book");
        let digest = control.stage_config(b"candidate").unwrap();
        let request = operation(&control, "book", "same-id", "creator", promote(&digest));
        let first = control.commit(&request).unwrap();
        assert_eq!(control.commit(&request).unwrap(), first);
        let mut changed = request;
        changed.actor_provenance = "different-authority".into();
        let error = control.commit(&changed).unwrap_err().to_string();
        assert!(error.contains("different bytes"), "{error}");
    }

    #[test]
    fn a_barrier_race_claims_only_the_expected_successor() {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let control = publish(objects, "book");
        let alpha = control.stage_config(b"alpha").unwrap();
        let beta = control.stage_config(b"beta").unwrap();
        let a = operation(&control, "book", "alpha", "creator", promote(&alpha));
        let b = operation(&control, "book", "beta", "creator", promote(&beta));
        let start = Arc::new(Barrier::new(3));
        let outcomes = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for request in [a, b] {
                let control = control.clone();
                let start = start.clone();
                handles.push(scope.spawn(move || {
                    start.wait();
                    control.commit(&request)
                }));
            }
            start.wait();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(control.read("book").unwrap().revision, 1);
    }

    #[test]
    fn simultaneous_exact_retries_receive_one_identical_receipt() {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let control = publish(objects, "book");
        let digest = control.stage_config(b"one reviewed candidate").unwrap();
        let request = operation(
            &control,
            "book",
            "same-operation",
            "creator",
            promote(&digest),
        );
        let start = Arc::new(Barrier::new(3));
        let receipts = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..2 {
                let control = control.clone();
                let request = request.clone();
                let start = start.clone();
                handles.push(scope.spawn(move || {
                    start.wait();
                    control.commit(&request).unwrap()
                }));
            }
            start.wait();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(receipts[0], receipts[1]);
        assert_eq!(control.read("book").unwrap().revision, 1);
    }

    #[test]
    fn independent_books_do_not_consume_each_others_revision_or_operation_id() {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let left = publish(objects.clone(), "left");
        BootstrapStore::new(objects.clone())
            .publish(&bootstrap("right", "creator"))
            .unwrap();
        let right = ControlStore::new(objects);
        for (control, book, bytes) in [
            (&left, "left", b"left".as_slice()),
            (&right, "right", b"right".as_slice()),
        ] {
            let digest = control.stage_config(bytes).unwrap();
            control
                .commit(&operation(
                    control,
                    book,
                    "same-operation",
                    "creator",
                    promote(&digest),
                ))
                .unwrap();
        }
        assert_eq!(left.read("left").unwrap().revision, 1);
        assert_eq!(right.read("right").unwrap().revision, 1);
    }

    #[derive(Default)]
    struct FaultStore {
        inner: MemoryStore,
        reads: Mutex<BTreeMap<String, Option<Vec<u8>>>>,
    }

    impl ObjectStore for FaultStore {
        fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
            self.inner.put_if_absent(key, body)
        }
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
            if let Some(value) = self.reads.lock().unwrap().get(key) {
                return Ok(value.clone());
            }
            self.inner.get(key)
        }
        fn list(&self, prefix: &str) -> Result<Vec<String>> {
            self.inner.list(prefix)
        }
    }

    #[test]
    fn missing_or_corrupt_durable_objects_refuse_despite_plausible_local_state() {
        let objects = Arc::new(FaultStore::default());
        let control = publish(objects.clone(), "book");
        let digest = control.stage_config(b"durable").unwrap();
        control
            .commit(&operation(
                &control,
                "book",
                "p",
                "creator",
                promote(&digest),
            ))
            .unwrap();
        let transition_key = ControlStore::transition_key("book", 1);
        let config_key = ControlStore::config_key(&digest);
        for (key, value) in [
            (transition_key.clone(), None),
            (transition_key, Some(vec![0xff])),
            (config_key.clone(), None),
            (config_key, Some(b"plausible but wrong".to_vec())),
        ] {
            objects.reads.lock().unwrap().insert(key.clone(), value);
            assert!(control.read("book").is_err(), "{key} did not refuse");
            objects.reads.lock().unwrap().clear();
        }
    }

    #[test]
    fn stale_predecessors_and_revoked_actors_fail_for_their_own_reason() {
        let objects: Arc<dyn ObjectStore> = Arc::new(MemoryStore::new());
        let control = publish(objects, "book");
        let stale = operation(
            &control,
            "book",
            "stale",
            "creator",
            member(membership_revision::Action::Grant, "guest"),
        );
        control
            .commit(&operation(
                &control,
                "book",
                "grant-admin",
                "creator",
                member(membership_revision::Action::Grant, "admin"),
            ))
            .unwrap();
        let conflict = control.commit(&stale).unwrap_err().to_string();
        assert!(conflict.contains("predecessor conflict"), "{conflict}");

        let revoked_request = operation(
            &control,
            "book",
            "creator-later",
            "creator",
            member(membership_revision::Action::Grant, "guest"),
        );
        control
            .commit(&operation(
                &control,
                "book",
                "revoke-creator",
                "admin",
                member(membership_revision::Action::Revoke, "creator"),
            ))
            .unwrap();
        let auth = control
            .commit(&revoked_request)
            .unwrap_err()
            .to_string();
        assert!(auth.contains("not a current book member"), "{auth}");
    }
}
