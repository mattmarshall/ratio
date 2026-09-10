"""The startup policy is operator data, empty by default and replaced atomically."""
import base64
import os
from pathlib import Path
import subprocess
import sys
import tempfile

entrypoint, app, flow = map(Path, sys.argv[1:])
source = entrypoint.read_text()
# Stop only the final server exec. Run the actual copy/seed/decode startup code.
source = source.replace(
    'exec /usr/local/bin/ratio watch --book "$BOOK" --port "${AWS_LWA_PORT:-8080}"',
    'exit 0',
)
assert 'exec /usr/local/bin/ratio watch' not in source
with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    book, funds = root / "book", root / "funds"
    book.mkdir()
    funds.mkdir()
    policy = funds / "CONNECT_GRANTS.pb"
    env = {**os.environ, "RATIO_BOOK": str(book), "RATIO_FUNDS": str(funds),
           "RATIO_DEMO_MEMBER": "", "RATIO_CONNECT_TEMPLATE_GRANTS_BASE64": ""}
    # Serialized ConnectTemplateGrants for a synthetic Personal client.
    payload = bytes.fromhex("0a1f0a0b636c69656e745f7465737410011a0e726563656976655f696e636f6d65")
    for configured in [base64.b64encode(payload).decode(), ""]:
        policy.write_bytes(b"stale")
        result = subprocess.run(["bash", "-c", source], env={**env,
            "RATIO_CONNECT_TEMPLATE_GRANTS_BASE64": configured}, capture_output=True)
        assert result.returncode == 0, result.stderr.decode()
        assert policy.read_bytes() == (payload if configured else b"")
    policy.write_bytes(b"previous")
    result = subprocess.run(["bash", "-c", source], env={**env,
        "RATIO_CONNECT_TEMPLATE_GRANTS_BASE64": "%%invalid%%"}, capture_output=True)
    assert result.returncode != 0, "invalid base64 must stop startup"
    assert policy.read_bytes() == b"previous", "a failed decode must not install a partial policy"

assert 'RATIO_CONNECT_TEMPLATE_GRANTS_BASE64: !Ref ConnectTemplateGrantsBase64' in app.read_text()
assert 'BOOK="${RATIO_BOOK:-/tmp/demo-book}"' in source
assert 'RATIO_BOOK: /tmp/demo-book' in app.read_text()
assert 'CONNECT_TEMPLATE_GRANTS_BASE64: ${{ vars.CONNECT_TEMPLATE_GRANTS_BASE64 }}' in flow.read_text()
assert 'ConnectTemplateGrantsBase64="${CONNECT_TEMPLATE_GRANTS_BASE64:-}"' in flow.read_text()
print("Connect policy startup: exact bytes, empty revocation, malformed refusal, deployment wiring")
