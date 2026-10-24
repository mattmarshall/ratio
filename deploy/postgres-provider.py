#!/usr/bin/env python3
"""Run a committed-source native cold comparison; credentials stay in env."""
import argparse
import os
from pathlib import Path
import subprocess

p=argparse.ArgumentParser()
p.add_argument("phase",choices=["seed","cold"])
p.add_argument("--attempt",required=True)
a=p.parse_args()
repo=Path(__file__).resolve().parent.parent
assert not subprocess.check_output(["git","status","--porcelain"],cwd=repo),"commit source before execution"
revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip()
run="20261023-f2fdf6e8"
logs=Path("/private/tmp")/f"ratio-provider-362-logs-{run}"
logs.mkdir(mode=0o700,exist_ok=True)
log=logs/f"{a.attempt}.log"
assert not log.exists(),"preserve prior attempt"
env=dict(os.environ,RATIO_CONFORMANCE_ROOT=f"/private/tmp/ratio-provider-362-{run}",RATIO_CONFORMANCE_PHASE=a.phase,RATIO_CONFORMANCE_ATTEMPT=a.attempt,RATIO_CONFORMANCE_REVISION=revision)
assert env.get("RATIO_NATIVE_PG_URL"),"set RATIO_NATIVE_PG_URL without sharing secrets"
r=subprocess.run([str(repo/"bazel-bin/crates/ratio-postgres-store/cold_fold_live_test"),"--exact","native_cold_fold_uses_the_same_5396_entry_journal_as_live_s3","--nocapture"],cwd=repo,env=env,capture_output=True,text=True)
text=f"source={revision} phase={a.phase} attempt={a.attempt}\n"+r.stdout+r.stderr
text=text.replace(env["RATIO_NATIVE_PG_URL"],"[redacted connection]")
log.write_text(text)
assert r.returncode==0,text[-2600:]
print(text[-2300:])
