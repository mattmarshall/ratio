"""The one error the SDK raises: a ``google.rpc.Status``."""

from __future__ import annotations

from typing import Any

# google.rpc.Code, by number. The REST body carries both the number and the
# name; gRPC carries the number and this table names it.
CODE_NAMES = {
    0: "OK",
    1: "CANCELLED",
    2: "UNKNOWN",
    3: "INVALID_ARGUMENT",
    4: "DEADLINE_EXCEEDED",
    5: "NOT_FOUND",
    6: "ALREADY_EXISTS",
    7: "PERMISSION_DENIED",
    8: "RESOURCE_EXHAUSTED",
    9: "FAILED_PRECONDITION",
    10: "ABORTED",
    11: "OUT_OF_RANGE",
    12: "UNIMPLEMENTED",
    13: "INTERNAL",
    14: "UNAVAILABLE",
    15: "DATA_LOSS",
    16: "UNAUTHENTICATED",
}


class RatioError(Exception):
    """A refused call: the ``google.rpc.Code`` number, its name, and the message.

    ``status`` is the string a caller matches on — ``"FAILED_PRECONDITION"`` for
    an entry that does not conserve value, ``"NOT_FOUND"``, ``"ALREADY_EXISTS"``.
    ``message`` is the server's own text: for a refused post, exactly what
    ``ratio post`` would have printed.
    """

    def __init__(self, code: int, message: str, status: str | None = None) -> None:
        self.code = code
        self.status = status or CODE_NAMES.get(code, "UNKNOWN")
        self.message = message
        super().__init__(f"{self.status}: {message}")

    @classmethod
    def from_json(cls, body: Any, http_status: int) -> "RatioError":
        """Build from a ``{"error": {"code", "message", "status"}}`` body, or
        from whatever a non-Ratio server put in front of us."""
        if isinstance(body, dict) and isinstance(body.get("error"), dict):
            e = body["error"]
            code = e.get("code")
            if not isinstance(code, int):
                code = 2
            return cls(code, str(e.get("message", "")), e.get("status"))
        return cls(2, f"HTTP {http_status}: {body!r}")
