import { RatioError } from "./errors.js";
import type { Transport } from "./transport.js";
import type { Account, ListAccountsResponse, ListTransactionsResponse, Posting, Transaction, TrialBalance } from "./types.js";
import * as wire from "./wire.js";

/**
 * The gRPC transport: the same seven calls over `@grpc/grpc-js`, with the wire
 * format from `wire.ts` — so `npm install @grpc/grpc-js` is the whole setup and
 * there is no stub to regenerate when the contract moves.
 */
/** The slice of `grpc.Client` this transport uses. */
interface ClientLike {
  makeUnaryRequest(
    method: string,
    serialize: (v: Uint8Array) => Buffer,
    deserialize: (b: Buffer) => Uint8Array,
    request: Uint8Array,
    callback: (err: { code: number; details: string } | null, value?: Uint8Array) => void,
  ): unknown;
  close(): void;
}

export class GrpcTransport implements Transport {
  private constructor(private readonly client: ClientLike) {}

  /** Connect to `endpoint` (`http://host:port` is plaintext, `https://` is TLS). */
  static async connect(endpoint: string): Promise<GrpcTransport> {
    let grpc: typeof import("@grpc/grpc-js");
    try {
      grpc = await import("@grpc/grpc-js");
    } catch {
      throw new Error("the gRPC transport needs @grpc/grpc-js — npm install @grpc/grpc-js — or use the REST transport, which needs nothing");
    }
    const tls = endpoint.startsWith("https://");
    const target = endpoint.replace(/^https?:\/\//, "");
    const creds = tls ? grpc.credentials.createSsl() : grpc.credentials.createInsecure();
    const client = new grpc.Client(target, creds);
    return new GrpcTransport(client as unknown as ClientLike);
  }

  private unary<T>(service: string, method: string, request: Uint8Array, decode: (b: Uint8Array) => T): Promise<T> {
    return new Promise((resolve, reject) => {
      this.client.makeUnaryRequest(
        `/ratio.v1.${service}/${method}`,
        (v) => Buffer.from(v),
        (b) => new Uint8Array(b),
        request,
        (err, value) => {
          if (err) reject(new RatioError(err.code, err.details ?? ""));
          else resolve(decode(value ?? new Uint8Array()));
        },
      );
    });
  }

  getTransaction(name: string): Promise<Transaction> {
    return this.unary("Ledger", "GetTransaction", wire.encodeGetRequest(name), wire.decodeTransaction);
  }
  listTransactions(parent: string, pageSize: number, pageToken: string): Promise<ListTransactionsResponse> {
    return this.unary("Ledger", "ListTransactions", wire.encodeListRequest(parent, pageSize, pageToken), wire.decodeListTransactionsResponse);
  }
  createTransaction(parent: string, postings: Posting[], transactionId: string, controlPlaneHash: string): Promise<Transaction> {
    const t: Transaction = { postings };
    if (controlPlaneHash) t.controlPlaneHash = controlPlaneHash;
    return this.unary("Ledger", "CreateTransaction", wire.encodeCreateTransactionRequest(parent, t, transactionId), wire.decodeTransaction);
  }
  getAccount(name: string): Promise<Account> {
    return this.unary("Chart", "GetAccount", wire.encodeGetRequest(name), wire.decodeAccount);
  }
  listAccounts(parent: string, pageSize: number, pageToken: string): Promise<ListAccountsResponse> {
    return this.unary("Chart", "ListAccounts", wire.encodeListRequest(parent, pageSize, pageToken), wire.decodeListAccountsResponse);
  }
  createAccount(parent: string, account: Account, accountId: string): Promise<Account> {
    return this.unary("Chart", "CreateAccount", wire.encodeCreateAccountRequest(parent, account, accountId), wire.decodeAccount);
  }
  getTrialBalance(name: string): Promise<TrialBalance> {
    return this.unary("Chart", "GetTrialBalance", wire.encodeGetRequest(name), wire.decodeTrialBalance);
  }
  close(): void {
    this.client.close();
  }
}
