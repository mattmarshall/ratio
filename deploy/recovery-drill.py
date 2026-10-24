#!/usr/bin/env python3
"""Approved synthetic AWS drill; build Rust with Bazel before invoking.

AWS credentials stay in child environments, never in evidence. Interchange
evidence uses RecoveryDrillEvidence; AWS responses retain their existing API
JSON schema. This operator script never discovers or opens a production book.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-env", required=True)
    parser.add_argument("--proto-dir", required=True)
    args = parser.parse_args()
    sys.path.insert(0, args.proto_dir)
    from ratio.storage.v1 import recovery_pb2 as pb

    settings = dict(line.split("=", 1) for line in Path(args.run_env).read_text().splitlines() if line)
    run_id = settings["RATIO_DRILL_ID"]
    lab = settings["RATIO_DRILL_BUCKET"]
    backup = settings["RATIO_DRILL_BACKUP_BUCKET"]
    assert lab.startswith("ratio-recovery-lab-") and backup.startswith("ratio-recovery-backup-") and lab != backup
    assert settings["AWS_PROFILE"] == "ratio" and settings["AWS_REGION"] == "us-east-1"
    repo = Path(__file__).resolve().parent.parent
    work = Path("/private/tmp") / ("ratio-approved-recovery-" + run_id)
    assert not work.exists(), "do not restart an existing run or discard its failure evidence"
    work.mkdir(mode=0o700)
    core = repo / "bazel-bin/crates/ratio/ratio"
    harness = repo / "bazel-bin/crates/ratio/recovery_live_test"
    assert core.is_file() and harness.is_file(), "build the CLI and manual test with Bazel first"
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True), "commit the reviewable rehearsal source before execution"
    aws_base = ["aws", "--profile", "ratio", "--region", "us-east-1"]
    exported = subprocess.run(aws_base + ["configure", "export-credentials", "--format", "process"], check=True, capture_output=True, text=True)
    credentials = json.loads(exported.stdout)
    child_env = {k: v for k, v in os.environ.items() if not k.startswith("RATIO_") and not k.startswith("AWS_ENDPOINT_URL")}
    child_env.update(AWS_ACCESS_KEY_ID=credentials["AccessKeyId"], AWS_SECRET_ACCESS_KEY=credentials["SecretAccessKey"], AWS_SESSION_TOKEN=credentials["SessionToken"], AWS_REGION="us-east-1", AWS_EC2_METADATA_DISABLED="true")
    source_prefix = f"recovery-lab/{run_id}/source/"
    restore_prefix = f"recovery-lab/{run_id}/restore/"
    snapshot_prefix = "snapshot/"
    evidence = pb.RecoveryDrillEvidence(run_id=run_id, kernel_revision=revision,
        operator="Codex under Matt Marshall's approved AWS session", verifier="Matt Marshall",
        source_location=f"s3://{lab}/{source_prefix}", backup_location=f"s3://{backup}/{snapshot_prefix}",
        restore_location=f"s3://{lab}/{restore_prefix}", 
        evidence_retention_days=7, verifier_accepted=False,
        limitations=["Synthetic fixture, not customer recovery acceptance", "Separate buckets in one AWS account and region; no cross-account disaster claim", "Synthetic Connect references contain no real provider tokens and establish byte custody only", "Synthetic gateway claims exercise application membership, not JWT signature validation"])

    evidence.rpo_target.FromMilliseconds(0)
    evidence.rto_target.FromMilliseconds(3_600_000)

    def redacted(text):
        for key in ("AccessKeyId", "SecretAccessKey", "SessionToken"):
            text = text.replace(credentials[key], "[redacted]")
        return text

    def command(name, argv, env=None, expected_error=None):
        result = subprocess.run([str(x) for x in argv], cwd=repo, env=env or child_env, capture_output=True, text=True)
        output = redacted(result.stdout + result.stderr)
        (work / (name + ".log")).write_text(output)
        if expected_error:
            assert result.returncode != 0 and expected_error in output, f"{name}: expected refusal {expected_error!r}: {output[-1800:]}"
        else:
            assert result.returncode == 0, f"{name}: {output[-1800:]}"
        return result.stdout

    def aws(name, *argv):
        text = command(name, aws_base + list(argv) + ["--output", "json"])
        return json.loads(text) if text.strip() else None

    def persist():
        (work / "drill-evidence.pb").write_bytes(evidence.SerializeToString(deterministic=True))

    def phase(name, action):
        item = evidence.phases.add(phase_id=name)
        item.started_time.GetCurrentTime()
        started = time.monotonic_ns()
        try:
            result = action()
            item.passed = True
            item.detail = "operator assertions passed; see preserved phase logs"
            return result
        except Exception as error:
            item.detail = redacted(str(error))
            raise
        finally:
            item.elapsed.FromMilliseconds((time.monotonic_ns() - started) // 1_000_000)
            persist()
            print(f"{name}: passed={item.passed} elapsed_ms={item.elapsed.ToMilliseconds()}", flush=True)

    def guards():
        assert aws("identity", "sts", "get-caller-identity")["Account"] == "320473299741"
        for bucket in (lab, backup):
            public = aws(bucket + "-public", "s3api", "get-public-access-block", "--bucket", bucket)
            assert all(public["PublicAccessBlockConfiguration"].values())
            encrypted = aws(bucket + "-encryption", "s3api", "get-bucket-encryption", "--bucket", bucket)
            assert encrypted["ServerSideEncryptionConfiguration"]["Rules"][0]["ApplyServerSideEncryptionByDefault"]["SSEAlgorithm"] == "AES256"
        assert aws("backup-versioning", "s3api", "get-bucket-versioning", "--bucket", backup)["Status"] == "Enabled"
        retention = aws("backup-retention", "s3api", "get-object-lock-configuration", "--bucket", backup)["ObjectLockConfiguration"]["Rule"]["DefaultRetention"]
        assert retention == {"Mode": "GOVERNANCE", "Days": 7}
        assert not aws("empty-source", "s3api", "list-objects-v2", "--bucket", lab, "--prefix", source_prefix).get("Contents")
        assert not aws("empty-restore", "s3api", "list-objects-v2", "--bucket", lab, "--prefix", restore_prefix).get("Contents")
        assert not aws("empty-backup", "s3api", "list-objects-v2", "--bucket", backup, "--prefix", snapshot_prefix).get("Contents")

    def harness_phase(name, prefix, root, record, baseline=None):
        env = dict(child_env, RATIO_DRILL_BUCKET=lab, RATIO_DRILL_PREFIX=prefix, RATIO_DRILL_ROOT=str(root),
            RATIO_DRILL_PHASE=name, RATIO_DRILL_ID=run_id, RATIO_DRILL_REVISION=revision,
            RATIO_DRILL_BACKUP=evidence.backup_location, RATIO_DRILL_RESTORE=evidence.restore_location,
            RATIO_DRILL_RECORD=str(record))
        if baseline:
            env["RATIO_DRILL_BASELINE"] = str(baseline)
        command(name, [harness, "--ignored", "--exact", "recovery_live::the_approved_synthetic_s3_rehearsal", "--nocapture"], env=env)
        return pb.RecoveryDrillEvidence.FromString(record.read_bytes())

    def recovery_env(backup_bucket, backup_prefix, destination_prefix=None):
        env = dict(child_env, RATIO_RECOVERY_BACKUP_BUCKET=backup_bucket, RATIO_RECOVERY_BACKUP_PREFIX=backup_prefix)
        if destination_prefix:
            env.update(RATIO_RECOVERY_DESTINATION_BUCKET=lab, RATIO_RECOVERY_DESTINATION_PREFIX=destination_prefix)
        return env

    try:
        phase("storage-guards", guards)
        baseline_path = work / "baseline.pb"
        baseline = phase("seed-and-quiesce", lambda: harness_phase("seed", source_prefix, work / "source-books", baseline_path))
        evidence.books.extend(baseline.books)
        evidence.external_inventory.extend(baseline.external_inventory)
        persist()
        # Only the seed process wrote this random namespace. It has exited;
        # no HTTP, MCP, ingest, or Connect worker is started by this drill.
        multipart = phase("interrupted-unpublished-write", lambda: aws("multipart-open", "s3api", "create-multipart-upload", "--bucket", lab, "--key", source_prefix + "alpha/journal/00000000000000000003"))
        partial = work / "unpublished-part"
        partial.write_bytes(b"unpublished synthetic journal body")
        aws("multipart-part", "s3api", "upload-part", "--bucket", lab, "--key", source_prefix + "alpha/journal/00000000000000000003", "--upload-id", multipart["UploadId"], "--part-number", "1", "--body", str(partial))
        capture_env = dict(recovery_env(backup, snapshot_prefix), RATIO_RECOVERY_SOURCE_BUCKET=lab, RATIO_RECOVERY_SOURCE_PREFIX=source_prefix)
        phase("clean-backup", lambda: command("capture", [core, "recovery", "capture"], env=capture_env))
        manifest_path = work / "capture.pb"
        manifest_head = aws("capture-manifest", "s3api", "get-object", "--bucket", backup, "--key", snapshot_prefix + "_recovery/capture.pb", str(manifest_path))
        manifest = pb.RecoveryManifest.FromString(manifest_path.read_bytes())
        assert list(manifest.objects) == list(baseline.external_inventory)
        assert manifest_head["ServerSideEncryption"] == "AES256" and manifest_head["ObjectLockMode"] == "GOVERNANCE"
        assert manifest_head.get("ObjectLockRetainUntilDate"), "backup retention is not merely a template promise"

        # Damage only a separately named lab copy, never the retained backup.
        damaged_prefix = f"recovery-lab/{run_id}/damaged-backup/"
        keys = [obj.key for obj in manifest.objects] + ["_recovery/capture.pb"]
        def damaged_copy():
            for number, key in enumerate(keys):
                aws(f"damage-copy-{number}", "s3api", "copy-object", "--bucket", lab, "--key", damaged_prefix + key, "--copy-source", backup + "/" + snapshot_prefix + key)
        phase("negative-copy-preparation", damaged_copy)
        journal_key = "alpha/journal/00000000000000000001"
        altered = work / "balanced-memo-alteration"
        aws("journal-before-mutation", "s3api", "get-object", "--bucket", lab, "--key", damaged_prefix + journal_key, str(altered))
        before = json.loads(altered.read_bytes())
        after = dict(before, memo="synthetic balanced memo corruption")
        assert {k: v for k, v in before.items() if k != "memo"} == {k: v for k, v in after.items() if k != "memo"}
        altered.write_text(json.dumps(after))
        aws("journal-mutation", "s3api", "put-object", "--bucket", lab, "--key", damaged_prefix + journal_key, "--body", str(altered))
        negative_env = recovery_env(lab, damaged_prefix, f"recovery-lab/{run_id}/refused-restore/")
        phase("balanced-corruption-refusal", lambda: command("corrupt-restore", [core, "recovery", "restore"], env=negative_env, expected_error="backup object differs from manifest"))
        assert not aws("refused-destination-stays-empty", "s3api", "list-objects-v2", "--bucket", lab, "--prefix", f"recovery-lab/{run_id}/refused-restore/").get("Contents")

        # Every protected plane must refuse missing and changed backup bytes.
        aws("repair-journal-mutation", "s3api", "copy-object", "--bucket", lab, "--key", damaged_prefix + journal_key, "--copy-source", backup + "/" + snapshot_prefix + journal_key)
        damaged_body = work / "corrupt-plane-body"
        damaged_body.write_bytes(b"synthetic corrupt evidence")
        for plane in ("journal", "deliveries", "entities", "facts", "actions", "explanations", "closes", "reports", "changes", "nav-strikes", "proposals"):
            key = next(obj.key for obj in manifest.objects if obj.key.startswith("alpha/" + plane + "/"))
            aws("remove-" + plane, "s3api", "delete-object", "--bucket", lab, "--key", damaged_prefix + key)
            phase("missing-" + plane + "-refusal", lambda plane=plane: command("missing-restore-" + plane, [core, "recovery", "restore"], env=negative_env, expected_error="backup object is missing"))
            aws("corrupt-" + plane, "s3api", "put-object", "--bucket", lab, "--key", damaged_prefix + key, "--body", str(damaged_body))
            phase("corrupt-" + plane + "-refusal", lambda plane=plane: command("corrupt-restore-" + plane, [core, "recovery", "restore"], env=negative_env, expected_error="backup object differs from manifest"))
            aws("repair-" + plane, "s3api", "copy-object", "--bucket", lab, "--key", damaged_prefix + key, "--copy-source", backup + "/" + snapshot_prefix + key)

        failure_started = time.monotonic_ns()
        def lose_source():
            # No production location is accepted by any command above.
            command("remove-source-objects", aws_base + ["s3", "rm", f"s3://{lab}/{source_prefix}", "--recursive"])
            shutil.rmtree(work / "source-books")
            aws("abort-unpublished", "s3api", "abort-multipart-upload", "--bucket", lab, "--key", source_prefix + "alpha/journal/00000000000000000003", "--upload-id", multipart["UploadId"])
            assert not aws("source-loss-confirmed", "s3api", "list-objects-v2", "--bucket", lab, "--prefix", source_prefix).get("Contents")
        phase("source-and-serving-root-loss", lose_source)
        phase("isolated-restore", lambda: command("restore", [core, "recovery", "restore"], env=recovery_env(backup, snapshot_prefix, restore_prefix)))
        restored = phase("fresh-process-figures-digests-membership", lambda: harness_phase("verify", restore_prefix, work / "restored-books", work / "restored.pb", baseline_path))
        assert list(restored.books) == list(baseline.books)
        rto = (time.monotonic_ns() - failure_started) // 1_000_000
        assert rto <= evidence.rto_target.ToMilliseconds(), f"RTO exceeded: {rto} ms"
        measurement = evidence.phases.add(phase_id="measured-rto-and-rpo", passed=True,
            detail="RPO 0: every acknowledged captured object and alpha prefix 2 / beta prefix 1 reproduced; RTO measured from source-loss start through fresh-process figure/access verification")
        measurement.started_time.GetCurrentTime()
        measurement.elapsed.FromMilliseconds(rto)
        persist()
        # Evidence is separate from snapshot/, whose exact object set is verified.
        for path in sorted(work.iterdir()):
            if path.is_file():
                aws("preserve-" + path.name, "s3api", "put-object", "--bucket", backup, "--key", "evidence/" + path.name, "--body", str(path), "--if-none-match", "*")
        print(f"SYNTHETIC_LIVE_S3 automated_checks=passed rpo=0 rto_ms={rto} verifier=Matt_Marshall acceptance=pending evidence=s3://{backup}/evidence/", flush=True)
    except Exception:
        persist()
        print(f"FAILED: preserved local evidence at {work}; do not discard or automatically restart", file=sys.stderr, flush=True)
        raise


if __name__ == "__main__":
    main()
