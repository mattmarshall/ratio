#!/usr/bin/env python3
"""Supplemental real CLI/HTTP/stdio MCP storage probes on a lab snapshot copy."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-env", required=True)
    parser.add_argument("--proto-dir", required=True)
    args = parser.parse_args()
    sys.path.insert(0, args.proto_dir)
    from ratio.storage.v1 import recovery_pb2 as pb, evidence_pb2 as stored
    settings = dict(line.split("=", 1) for line in Path(args.run_env).read_text().splitlines() if line)
    run = settings["RATIO_DRILL_ID"]
    lab, backup = settings["RATIO_DRILL_BUCKET"], settings["RATIO_DRILL_BACKUP_BUCKET"]
    assert lab.startswith("ratio-recovery-lab-") and backup.startswith("ratio-recovery-backup-")
    parent = Path("/private/tmp") / ("ratio-approved-recovery-" + run)
    work = parent / "writer-probes"
    assert not work.exists(), "preserve an existing attempt instead of restarting"
    work.mkdir(mode=0o700)
    repo = Path(__file__).resolve().parent.parent
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True)
    core = str(repo / "bazel-bin/crates/ratio/ratio")
    aws = ["aws", "--profile", "ratio", "--region", "us-east-1"]
    credentials = json.loads(subprocess.check_output(aws + ["configure", "export-credentials", "--format", "process"], text=True))
    env = {k: v for k, v in os.environ.items() if not k.startswith("RATIO_") and not k.startswith("AWS_ENDPOINT_URL")}
    env.update(AWS_ACCESS_KEY_ID=credentials["AccessKeyId"], AWS_SECRET_ACCESS_KEY=credentials["SecretAccessKey"], AWS_SESSION_TOKEN=credentials["SessionToken"], AWS_REGION="us-east-1", AWS_EC2_METADATA_DISABLED="true")
    prefix = f"recovery-lab/{run}/writers/"
    serving = work / "serving"
    configured = dict(env, RATIO_JOURNAL_BUCKET=lab, RATIO_JOURNAL_PREFIX=prefix)
    record = pb.RecoveryDrillEvidence.FromString((parent / "continuation-evidence.pb").read_bytes())
    record.limitations.append("Supplemental writer probes use a distinct lab snapshot copy and loopback HTTP; they do not claim production gateway JWT verification.")

    def redact(text):
        for key in ("AccessKeyId", "SecretAccessKey", "SessionToken"):
            text = text.replace(credentials[key], "[redacted]")
        return text

    def command(label, argv, selected=None, stdin=None, refusal=None):
        result = subprocess.run([str(x) for x in argv], cwd=repo, env=selected or env, input=stdin, capture_output=True, text=True)
        output = redact(result.stdout + result.stderr)
        (work / (label + ".log")).write_text(output)
        if refusal:
            assert result.returncode != 0 and refusal in output, output[-1800:]
        else:
            assert result.returncode == 0, output[-1800:]
        return result.stdout

    def api(label, *argv):
        text = command(label, aws + list(argv) + ["--output", "json"])
        return json.loads(text) if text.strip() else None

    def phase(label, action):
        item = record.phases.add(phase_id=label)
        item.started_time.GetCurrentTime()
        start = time.monotonic_ns()
        try:
            value = action()
            item.passed = True
            item.detail = "real configured writer assertions passed; see preserved logs"
            return value
        except Exception as error:
            item.detail = redact(str(error))
            raise
        finally:
            item.elapsed.FromMilliseconds((time.monotonic_ns() - start) // 1_000_000)
            (work / "writer-evidence.pb").write_bytes(record.SerializeToString(deterministic=True))
            print(f"{label}: passed={item.passed}", flush=True)

    def mcp(label, book, tool, arguments):
        request = {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": tool, "arguments": arguments}}
        output = command(label, [core, "mcp", "--book", serving / book], configured, stdin=json.dumps(request) + "\n")
        response = json.loads(output)
        assert "error" not in response and not response["result"].get("isError"), response
        return response

    def objects():
        return api("writer-object-list", "s3api", "list-objects-v2", "--bucket", lab, "--prefix", prefix).get("Contents", [])

    assert not objects(), "writer probe destination must start empty"
    restore = dict(env, RATIO_RECOVERY_BACKUP_BUCKET=backup, RATIO_RECOVERY_BACKUP_PREFIX="snapshot/", RATIO_RECOVERY_DESTINATION_BUCKET=lab, RATIO_RECOVERY_DESTINATION_PREFIX=prefix)
    phase("writer-snapshot-copy", lambda: command("restore-writer-copy", [core, "recovery", "restore"], restore))
    event = work / "cli-entry.json"
    event.write_text(json.dumps([{"id": "live-cli-post", "postings": [{"dim": 2, "amount": 2500}, {"dim": 20, "amount": -2500}], "trade_date": "2026-07-02"}]))
    phase("live-cli-post", lambda: command("cli-post", [core, "post", event, "--book", serving / "alpha"], configured))
    phase("live-cli-strike", lambda: command("cli-strike", [core, "strike", "--as-of", "2026-07-03", "--book", serving / "alpha"], configured))
    phase("live-stdio-mcp-post", lambda: mcp("mcp-post", "beta", "post_events", {"events": [{"rule": "household_income", "id": "live-mcp-post", "amount": 1000}]}))
    proposal = '[[rule]]\nid="recovery_draft"\nkind="trade"\n[[rule.posting]]\naccount=1\nweight=1\n[[rule.posting]]\naccount=20\nweight=-1\n'
    phase("live-stdio-mcp-draft", lambda: mcp("mcp-draft", "beta", "propose_rule", {"toml": proposal}))

    def http_write():
        server = subprocess.Popen([core, "server", "--addr", "127.0.0.1:0", "--book", str(serving / "beta")], cwd=repo, env=configured, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            line = server.stdout.readline()
            assert " on http://127.0.0.1:" in line, line
            url = line.rsplit(" on ", 1)[1].strip()
            def call(label, body, transaction_id):
                payload = json.dumps(body).encode()
                request = urllib.request.Request(url + "/v1/books/beta/transactions?transactionId=" + transaction_id, data=payload, headers={"Content-Type": "application/json"})
                try:
                    with urllib.request.urlopen(request, timeout=30) as response:
                        status, data = response.status, response.read().decode()
                except urllib.error.HTTPError as error:
                    status, data = error.code, error.read().decode()
                (work / (label + ".log")).write_text(data)
                return status, json.loads(data)
            status, response = call("http-post", {"postings": [{"dim": "2", "amount": "1000"}, {"dim": "20", "amount": "-1000"}]}, "live-http-post")
            assert status == 200 and response["name"].endswith("/live-http-post"), (status, response)
            status, _ = call("http-unbalanced-refusal", {"postings": [{"dim": "2", "amount": "999"}, {"dim": "20", "amount": "-1000"}]}, "refused-http-post")
            assert status == 400, status
        finally:
            server.terminate()
            out, err = server.communicate(timeout=30)
            (work / "http-server.log").write_text(redact(line + out + err))
    phase("live-loopback-http-post-and-refusal", http_write)

    def fail_closed():
        before = {p.relative_to(serving): hashlib.sha256(p.read_bytes()).digest() for p in serving.rglob("*") if p.is_file()}
        unavailable = dict(configured, RATIO_JOURNAL_BUCKET="ratio-recovery-lab-missing-" + run)
        commands = [("init", ["init"]), ("post", ["post", str(event)]), ("strike", ["strike"]), ("close", ["close", "--through", "2026-07-31"]), ("approve", ["approve", "unapproved-draft"]), ("accept", ["accept", "synthetic-break", "--because", "fixture"]), ("mcp", ["mcp"])]
        for label, cmd in commands:
            command("live-refuse-" + label, [core] + cmd + ["--book", str(serving / "alpha")], unavailable, stdin="", refusal="NoSuchBucket")
        after = {p.relative_to(serving): hashlib.sha256(p.read_bytes()).digest() for p in serving.rglob("*") if p.is_file()}
        assert before == after, "a failed configured writer changed local book bytes"
    phase("all-cli-mcp-writers-refuse-live-s3-failure", fail_closed)

    shutil.rmtree(serving)
    phase("cold-cli-read-after-serving-root-loss", lambda: command("cold-cli", [core, "balance", "--book", serving / "alpha"], configured))
    phase("cold-mcp-read-after-serving-root-loss", lambda: mcp("cold-mcp", "beta", "trial_balance", {}))
    all_objects = objects()
    for book, expected in (("alpha", ["capital-1", "capital-2", "live-cli-post"]), ("beta", ["beta-capital", "live-mcp-post", "live-http-post"])):
        journal = sorted(obj["Key"] for obj in all_objects if obj["Key"].startswith(prefix + book + "/journal/"))
        ids = []
        for index, key in enumerate(journal):
            path = work / f"{book}-journal-{index}.json"
            api(f"read-{book}-{index}", "s3api", "get-object", "--bucket", lab, "--key", key, str(path))
            ids.append(json.loads(path.read_bytes())["id"])
        assert ids == expected, (book, ids)
    nav_keys = [obj["Key"] for obj in all_objects if obj["Key"].startswith(prefix + "alpha/nav-strikes/")]
    navs = []
    for index, key in enumerate(nav_keys):
        path = work / f"alpha-nav-{index}.pb"
        api(f"read-nav-{index}", "s3api", "get-object", "--bucket", lab, "--key", key, str(path))
        navs.append(stored.StoredNavStrike.FromString(path.read_bytes()))
    assert any(nav.net_asset_value == 62500 and nav.journal_position == 3 for nav in navs)
    assert sum(obj["Key"].startswith(prefix + "beta/proposals/") for obj in all_objects) == 1
    record.phases.add(phase_id="writer-cold-s3-readback", passed=True, detail="All three protocols' exact journal IDs survived root loss; CLI strike NAV 62500 at prefix 3 and the unapproved MCP draft survived in S3.")
    (work / "writer-evidence.pb").write_bytes(record.SerializeToString(deterministic=True))
    archive = parent / "writer-probes.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
        for path in sorted(work.iterdir()):
            if path.is_file():
                bundle.write(path, path.name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    api("preserve-writer-evidence", "s3api", "put-object", "--bucket", backup, "--key", "evidence/writer-probes.zip", "--body", str(archive), "--if-none-match", "*")
    readback = archive.with_suffix(".readback.zip")
    api("writer-evidence-readback", "s3api", "get-object", "--bucket", backup, "--key", "evidence/writer-probes.zip", str(readback))
    assert hashlib.sha256(readback.read_bytes()).hexdigest() == digest
    print(f"LIVE_WRITER_PROBES passed evidence_sha256={digest} verifier_acceptance=pending", flush=True)


if __name__ == "__main__":
    main()
