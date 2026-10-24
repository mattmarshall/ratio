#!/usr/bin/env python3
"""Explicit continuation of the preserved audit-expectation failure.

Never creates a new snapshot, removes a previous attempt, or resets RTO.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", required=True)
    parser.add_argument("--run-env", required=True)
    parser.add_argument("--proto-dir", required=True)
    args = parser.parse_args()
    sys.path.insert(0, args.proto_dir)
    from ratio.storage.v1 import recovery_pb2 as pb
    work = Path(args.work)
    original = pb.RecoveryDrillEvidence.FromString((work / "drill-evidence.pb").read_bytes())
    assert original.phases[-1].phase_id == "fresh-process-figures-digests-membership" and not original.phases[-1].passed
    assert "changes::read_with_store" in original.phases[-1].detail and "3 vs 2" in original.phases[-1].detail
    loss = next(p for p in original.phases if p.phase_id == "source-and-serving-root-loss")
    assert loss.passed and next(p for p in original.phases if p.phase_id == "isolated-restore").passed
    settings = dict(line.split("=", 1) for line in Path(args.run_env).read_text().splitlines() if line)
    assert settings["RATIO_DRILL_ID"] == original.run_id
    lab = settings["RATIO_DRILL_BUCKET"]
    backup = settings["RATIO_DRILL_BACKUP_BUCKET"]
    assert lab.startswith("ratio-recovery-lab-") and backup.startswith("ratio-recovery-backup-")
    repo = Path(__file__).resolve().parent.parent
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    aws = ["aws", "--profile", "ratio", "--region", "us-east-1"]
    credentials = json.loads(subprocess.check_output(aws + ["configure", "export-credentials", "--format", "process"], text=True))
    env = {k: v for k, v in os.environ.items() if not k.startswith("RATIO_") and not k.startswith("AWS_ENDPOINT_URL")}
    record_path = work / "restored-audit-continuation.pb"
    assert not record_path.exists()
    env.update(AWS_ACCESS_KEY_ID=credentials["AccessKeyId"], AWS_SECRET_ACCESS_KEY=credentials["SecretAccessKey"], AWS_SESSION_TOKEN=credentials["SessionToken"], AWS_REGION="us-east-1", AWS_EC2_METADATA_DISABLED="true",
        RATIO_DRILL_BUCKET=lab, RATIO_DRILL_PREFIX=f"recovery-lab/{original.run_id}/restore/",
        RATIO_DRILL_ROOT=str(work / "restored-books-audit-continuation"), RATIO_DRILL_PHASE="verify",
        RATIO_DRILL_ID=original.run_id, RATIO_DRILL_REVISION=revision, RATIO_DRILL_BACKUP=original.backup_location,
        RATIO_DRILL_RESTORE=original.restore_location, RATIO_DRILL_RECORD=str(record_path), RATIO_DRILL_BASELINE=str(work / "baseline.pb"))
    phase = original.phases.add(phase_id="explicit-audit-expectation-continuation")
    phase.started_time.GetCurrentTime()
    started = time.monotonic_ns()
    result = subprocess.run([str(repo / "bazel-bin/crates/ratio/recovery_live_test"), "--ignored", "--exact", "recovery_live::the_approved_synthetic_s3_rehearsal", "--nocapture"], env=env, cwd=repo, capture_output=True, text=True)
    output = result.stdout + result.stderr
    for key in ("AccessKeyId", "SecretAccessKey", "SessionToken"):
        output = output.replace(credentials[key], "[redacted]")
    (work / "audit-continuation.log").write_text(output)
    phase.elapsed.FromMilliseconds((time.monotonic_ns() - started) // 1_000_000)
    phase.passed = result.returncode == 0
    phase.detail = f"Helper source {revision}; original failure and original RTO start retained. " + output[-1800:]
    rto = (time.time_ns() - loss.started_time.ToNanoseconds()) // 1_000_000
    measurement = original.phases.add(phase_id="rto-including-original-failure-and-repair")
    measurement.started_time.CopyFrom(loss.started_time)
    measurement.elapsed.FromMilliseconds(rto)
    measurement.passed = phase.passed and 0 <= rto <= original.rto_target.ToMilliseconds()
    measurement.detail = "No timer reset. Includes initial restore, failed assertion, diagnosis, rebuild, and another absent-root verification. RPO 0 requires exact captured object and pinned-figure equality in the successful verifier."
    original.verifier_accepted = False
    (work / "continuation-evidence.pb").write_bytes(original.SerializeToString(deterministic=True))
    archive = work.parent / ("ratio-recovery-attempt-2-" + original.run_id + ".zip")
    assert not archive.exists()
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for path in sorted(work.iterdir()):
            if path.is_file():
                bundle.write(path, path.name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    key = "evidence/attempt-2.zip"
    subprocess.run(aws + ["s3api", "put-object", "--bucket", backup, "--key", key, "--body", str(archive), "--if-none-match", "*", "--query", "{VersionId:VersionId}", "--output", "json"], check=True)
    readback = archive.with_suffix(".readback.zip")
    head = json.loads(subprocess.check_output(aws + ["s3api", "get-object", "--bucket", backup, "--key", key, str(readback), "--output", "json"], text=True))
    assert hashlib.sha256(readback.read_bytes()).hexdigest() == digest
    assert head["ServerSideEncryption"] == "AES256" and head["ObjectLockMode"] == "GOVERNANCE"
    print(f"continuation_passed={phase.passed} rto_ms={rto} rto_target_met={measurement.passed} evidence_sha256={digest} verifier_acceptance=pending", flush=True)
    assert phase.passed and measurement.passed, output[-2000:]


if __name__ == "__main__":
    main()
