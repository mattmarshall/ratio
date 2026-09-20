import { RatioError } from "./errors.js";
import type { Transport } from "./transport.js";
import type { Account, ListAccountsResponse, ListTransactionsResponse, Posting, Transaction, TrialBalance } from "./types.js";

export type FetchLike = (input: string, init: { method: string; headers: Record<string, string>; body?: string }) => Promise<{
  status: number;
  text(): Promise<string>;
}>;

/** The REST transport: the contract's `google.api.http` bindings over `fetch`. No dependency. */
export class RestTransport implements Transport {
  private readonly endpoint: string;
  private readonly fetchImpl: FetchLike;

  constructor(endpoint: string, fetchImpl?: FetchLike) {
    this.endpoint = endpoint.replace(/\/+$/, "");
    const f = fetchImpl ?? (globalThis.fetch as unknown as FetchLike | undefined);
    if (!f) throw new Error("no fetch available: pass one to RestTransport, or use Node 18+");
    this.fetchImpl = f;
  }

  getTransaction(name: string): Promise<Transaction> {
    return this.call("GET", `/v1/${name}`);
  }

  listTransactions(parent: string, pageSize: number, pageToken: string): Promise<ListTransactionsResponse> {
    return this.call("GET", `/v1/${parent}/transactions`, page(pageSize, pageToken));
  }

  createTransaction(parent: string, postings: Posting[], transactionId: string, controlPlaneHash: string): Promise<Transaction> {
    const body: Transaction = { postings };
    if (controlPlaneHash) body.controlPlaneHash = controlPlaneHash;
    return this.call("POST", `/v1/${parent}/transactions`, transactionId ? { transactionId } : {}, body);
  }

  getAccount(name: string): Promise<Account> {
    return this.call("GET", `/v1/${name}`);
  }

  listAccounts(parent: string, pageSize: number, pageToken: string): Promise<ListAccountsResponse> {
    return this.call("GET", `/v1/${parent}/accounts`, page(pageSize, pageToken));
  }

  createAccount(parent: string, account: Account, accountId: string): Promise<Account> {
    return this.call("POST", `/v1/${parent}/accounts`, accountId ? { accountId } : {}, account);
  }

  getTrialBalance(name: string): Promise<TrialBalance> {
    return this.call("GET", `/v1/${name}`);
  }

  close(): void {}

  private async call<T>(method: string, path: string, query: Record<string, string> = {}, body?: unknown): Promise<T> {
    const qs = new URLSearchParams(query).toString();
    const url = this.endpoint + path + (qs ? `?${qs}` : "");
    const headers: Record<string, string> = { accept: "application/json" };
    const init: { method: string; headers: Record<string, string>; body?: string } = { method, headers };
    if (body !== undefined) {
      headers["content-type"] = "application/json";
      init.body = JSON.stringify(body);
    }
    let res: { status: number; text(): Promise<string> };
    try {
      res = await this.fetchImpl(url, init);
    } catch (e) {
      throw new RatioError(14, `${url}: ${(e as Error).message}`);
    }
    const text = await res.text();
    const parsed = text ? tryJson(text) : null;
    if (res.status < 200 || res.status >= 300) throw RatioError.fromBody(parsed, res.status);
    return parsed as T;
  }
}

function page(pageSize: number, pageToken: string): Record<string, string> {
  const q: Record<string, string> = {};
  if (pageSize) q.pageSize = String(pageSize);
  if (pageToken) q.pageToken = pageToken;
  return q;
}

function tryJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}
