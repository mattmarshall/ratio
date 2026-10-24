#!/usr/bin/env python3
"""Run one explicitly selected live S3 conformance fixture; preserve every log."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

parser=argparse.ArgumentParser()
parser.add_argument("phase",choices=["fixture"])
args=parser.parse_args()
repo=Path(__file__).resolve().parent.parent
assert not subprocess.check_output(["git","status","--porcelain"],cwd=repo), "commit source before execution"
revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip()
settings=dict(line.split("=",1) for line in Path("/private/tmp/ratio-approved-recovery-run.env").read_text().splitlines() if line)
run=settings["RATIO_DRILL_ID"]
logs=Path("/private/tmp")/("ratio-provider-359-logs-"+run)
logs.mkdir(mode=0o700,exist_ok=True)
log=logs/(args.phase+".log")
assert not log.exists(), "preserve the prior phase attempt"
aws=["aws","--profile","ratio","--region","us-east-1"]
creds=json.loads(subprocess.check_output(aws+["configure","export-credentials","--format","process"],text=True))
env={k:v for k,v in os.environ.items() if not k.startswith(("RATIO_","AWS_ENDPOINT_URL"))}
env.update(AWS_ACCESS_KEY_ID=creds["AccessKeyId"],AWS_SECRET_ACCESS_KEY=creds["SecretAccessKey"],AWS_SESSION_TOKEN=creds["SessionToken"],AWS_REGION="us-east-1",AWS_EC2_METADATA_DISABLED="true",RATIO_CONFORMANCE_BUCKET=settings["RATIO_DRILL_BUCKET"],RATIO_CONFORMANCE_PREFIX=f"conformance-lab/{run}/359/",RATIO_CONFORMANCE_ROOT=f"/private/tmp/ratio-provider-359-{run}",RATIO_CONFORMANCE_PHASE=args.phase,RATIO_HEIGHT_SOURCE_ROOT=f"/private/tmp/ratio-provider-357-{run}",RATIO_CONFORMANCE_REVISION=revision)
print(f"starting live provider conformance phase={args.phase} source={revision}",flush=True)
result=subprocess.run([str(repo/"bazel-bin/crates/ratio/provider_live_test"),"--ignored","--exact","provider_live::the_registered_fixture_conforms_against_live_s3","--nocapture"],cwd=repo,env=env,capture_output=True,text=True)
text=result.stdout+result.stderr
for key in ("AccessKeyId","SecretAccessKey","SessionToken"):
    text=text.replace(creds[key],"[redacted]")
log.write_text(text)
assert result.returncode==0,text[-2500:]
pattern=r"projection_checkpoint hit=[01] miss=[01](?: reason=(?:missing|corrupt|digest_mismatch|version_refused|prefix_mismatch))? checkpoint_prefix=\d+ journal_height=\d+ tail_length=\d+"
for line in text.splitlines():
    if line.startswith("projection_checkpoint "):
        assert re.fullmatch(pattern,line),"checkpoint telemetry contains undeclared content"
print(text[-2200:],flush=True)
print(f"phase={args.phase} passed; telemetry privacy checked",flush=True)
