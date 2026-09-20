/** google.rpc.Code, by number. */
export const CODE_NAMES: Record<number, string> = {
  0: "OK", 1: "CANCELLED", 2: "UNKNOWN", 3: "INVALID_ARGUMENT", 4: "DEADLINE_EXCEEDED",
  5: "NOT_FOUND", 6: "ALREADY_EXISTS", 7: "PERMISSION_DENIED", 8: "RESOURCE_EXHAUSTED",
  9: "FAILED_PRECONDITION", 10: "ABORTED", 11: "OUT_OF_RANGE", 12: "UNIMPLEMENTED",
  13: "INTERNAL", 14: "UNAVAILABLE", 15: "DATA_LOSS", 16: "UNAUTHENTICATED",
};

/**
 * A refused call: the google.rpc.Code number, its name, and the server's message.
 *
 * `status` is what to match on — `"FAILED_PRECONDITION"` for an entry that does
 * not conserve value, `"NOT_FOUND"`, `"ALREADY_EXISTS"`. `message` is the
 * server's own text: for a refused post, exactly what `ratio post` prints.
 */
export class RatioError extends Error {
  readonly code: number;
  readonly status: string;

  constructor(code: number, message: string, status?: string) {
    const name = status ?? CODE_NAMES[code] ?? "UNKNOWN";
    super(`${name}: ${message}`);
    this.name = "RatioError";
    this.code = code;
    this.status = name;
  }

  /** From a `{"error": {"code", "message", "status"}}` body, or whatever a non-Ratio server put in front of us. */
  static fromBody(body: unknown, httpStatus: number): RatioError {
    const error = (body as { error?: Record<string, unknown> } | null)?.error;
    if (error && typeof error === "object") {
      const code = typeof error.code === "number" ? error.code : 2;
      return new RatioError(code, String(error.message ?? ""), typeof error.status === "string" ? error.status : undefined);
    }
    return new RatioError(2, `HTTP ${httpStatus}: ${typeof body === "string" ? body : JSON.stringify(body)}`);
  }
}
