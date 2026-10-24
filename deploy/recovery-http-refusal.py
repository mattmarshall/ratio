#!/usr/bin/env python3
"""Prove configured HTTP startup refuses unavailable S3 without local fallback."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile

sys.path.insert(0, "/private/tmp/ratio-recovery-proto")
from ratio.storage.v1 import recovery_pb2 as pb

repo = Path(__file__).resolve().parent.parent
assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=repo)
revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
settings = dict(line.split("=", 1) for line in Path("/private/tmp/ratio-approved-recovery-run.env").read_text().splitlines() if line)
run = settings["RATIO_DRILL_ID"]
assert settings["RATIO_DRILL_BUCKET"].startswith("ratio-recovery-lab-")
assert settings["RATIO_DRILL_BACKUP_BUCKET"].startswith("ratio-recovery-backup-")
work = Path("/private/tmp") / ("ratio-approved-recovery-" + run) / "writer-probes"
output = work / "http-unavailable-backend.log"
assert not output.exists(), "preserve the prior HTTP refusal attempt"
book = work / "serving" / "beta"
assert book.is_dir()
aws = ["aws", "--profile", "ratio", "--region", "us-east-1"]
credentials = json.loads(subprocess.check_output(aws + ["configure", "export-credentials", "--format", "process"], text=True))
env = {k: v for k, v in os.environ.items() if not k.startswith(("RATIO_", "AWS_ENDPOINT_URL"))}
env.update(AWS_ACCESS_KEY_ID=credentials["AccessKeyId"], AWS_SECRET_ACCESS_KEY=credentials["SecretAccessKey"], AWS_SESSION_TOKEN=credentials["SessionToken"], AWS_REGION="us-east-1", AWS_EC2_METADATA_DISABLED="true", RATIO_JOURNAL_BUCKET="ratio-recovery-lab-missing-" + run, RATIO_JOURNAL_PREFIX=f"recovery-lab/{run}/writers/")

def snapshot():
    return {p.relative_to(book): hashlib.sha256(p.read_bytes()).digest() for p in book.rglob("*") if p.is_file()}

before = snapshot()
evidence = pb.RecoveryDrillEvidence.FromString((work / "writer-evidence.pb").read_bytes())
phase = evidence.phases.add(phase_id="live-http-unavailable-backend-refusal")
phase.started_time.GetCurrentTime()
try:
    result = subprocess.run([str(repo / "bazel-bin/crates/ratio/ratio"), "server", "--addr", "127.0.0.1:0", "--book", str(book)], cwd=repo, env=env, capture_output=True, text=True, timeout=30)
    text = result.stdout + result.stderr
    for key in ("AccessKeyId", "SecretAccessKey", "SessionToken"):
        text = text.replace(credentials[key], "[redacted]")
    output.write_text(text)
    assert result.returncode != 0 and "durable book publication is unavailable; refusing local fallback" in text, text
    assert " on http://" not in text, "unavailable storage admitted HTTP traffic"
    assert snapshot() == before, "failed HTTP startup changed local book bytes"
    phase.passed = True
    phase.detail = f"Source {revision}: HTTP refused unavailable durable publication before binding; local bytes unchanged."
finally:
    (work / "http-refusal-evidence.pb").write_bytes(evidence.SerializeToString(deterministic=True))
archive = work.parent / "http-refusal-evidence.zip"
with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as bundle:
    for path in (output, work / "http-refusal-evidence.pb"):
        bundle.write(path, path.name)
key = "evidence/" + archive.name
subprocess.run(aws + ["s3api", "put-object", "--bucket", settings["RATIO_DRILL_BACKUP_BUCKET"], "--key", key, "--body", str(archive), "--if-none-match", "*"], check=True, stdout=(work / "http-refusal-upload.json").open("w"))
readback = archive.with_suffix(".readback.zip")
subprocess.run(aws + ["s3api", "get-object", "--bucket", settings["RATIO_DRILL_BACKUP_BUCKET"], "--key", key, str(readback)], check=True, stdout=(work / "http-refusal-readback.json").open("w"))
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
assert hashlib.sha256(readback.read_bytes()).hexdigest() == digest
print(f"HTTP unavailable-backend refusal passed; evidence_sha256={digest}")
