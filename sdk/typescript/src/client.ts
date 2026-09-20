import { RestTransport, type FetchLike } from "./rest.js";
import type { Transport } from "./transport.js";
import type { Account, AccountType, ListAccountsResponse, ListTransactionsResponse, Posting, Transaction, TrialBalance } from "./types.js";
import { int64 } from "./types.js";

export interface RatioOptions {
  /** `http://host:port` — one `ratio server`, gRPC and REST on the same port. */
  endpoint: string;
  /** The segment after `books/`: the directory name the server was started on. */
  book: string;
  /** `"rest"` (default; needs nothing) or `"grpc"` (needs `@grpc/grpc-js`). */
  transport?: "rest" | "grpc";
  /** A `fetch` to use for REST — for a test, or a runtime without a global one. */
  fetch?: FetchLike;
}

/** `ratio.transactions` — the journal. */
export class Transactions {
  constructor(private readonly t: Transport, private readonly parent: string) {}

  /**
   * Post a transaction. Refused with `FAILED_PRECONDITION` unless every conserved
   * dimension nets to zero; `ALREADY_EXISTS` if `id` is taken; `FAILED_PRECONDITION`
   * if `controlPlaneHash` is not the active configuration.
   */
  create(postings: Posting[], opts: { id?: string; controlPlaneHash?: string } = {}): Promise<Transaction> {
    return this.t.createTransaction(this.parent, postings, opts.id ?? "", opts.controlPlaneHash ?? "");
  }

  get(id: string): Promise<Transaction> {
    return this.t.getTransaction(`${this.parent}/transactions/${id}`);
  }

  list(pageSize = 0, pageToken = ""): Promise<ListTransactionsResponse> {
    return this.t.listTransactions(this.parent, pageSize, pageToken);
  }

  /** Every transaction, in posting order, page by page. */
  async *[Symbol.asyncIterator](): AsyncGenerator<Transaction> {
    let token = "";
    for (;;) {
      const page = await this.list(0, token);
      yield* page.transactions;
      if (!page.nextPageToken) return;
      token = page.nextPageToken;
    }
  }
}

/** `ratio.accounts` — the chart. */
export class Accounts {
  constructor(private readonly t: Transport, private readonly parent: string) {}

  /** Add an account. The id is the dimension; the normal side comes back derived from the type. */
  create(dim: bigint | number, displayName: string, accountType: AccountType): Promise<Account> {
    return this.t.createAccount(this.parent, { dim: int64(dim), displayName, accountType }, "");
  }

  get(dim: bigint | number): Promise<Account> {
    return this.t.getAccount(`${this.parent}/accounts/${int64(dim)}`);
  }

  list(pageSize = 0, pageToken = ""): Promise<ListAccountsResponse> {
    return this.t.listAccounts(this.parent, pageSize, pageToken);
  }

  async *[Symbol.asyncIterator](): AsyncGenerator<Account> {
    let token = "";
    for (;;) {
      const page = await this.list(0, token);
      yield* page.accounts;
      if (!page.nextPageToken) return;
      token = page.nextPageToken;
    }
  }
}

/** A `ratio server`, scoped to `books/{book}`. */
export class Ratio {
  readonly parent: string;
  readonly transactions: Transactions;
  readonly accounts: Accounts;

  private constructor(private readonly transport: Transport, book: string) {
    this.parent = `books/${book}`;
    this.transactions = new Transactions(transport, this.parent);
    this.accounts = new Accounts(transport, this.parent);
  }

  /** REST, synchronously — the default. */
  static rest(opts: Omit<RatioOptions, "transport">): Ratio {
    return new Ratio(new RestTransport(opts.endpoint, opts.fetch), opts.book);
  }

  /** Either transport; gRPC connects lazily and needs `@grpc/grpc-js`. */
  static async connect(opts: RatioOptions): Promise<Ratio> {
    if ((opts.transport ?? "rest") === "rest") return Ratio.rest(opts);
    const { GrpcTransport } = await import("./grpc.js");
    return new Ratio(await GrpcTransport.connect(opts.endpoint), opts.book);
  }

  /** Totals per side and their difference — zero for any book the kernel admitted — with a row per (account, currency). */
  trialBalance(): Promise<TrialBalance> {
    return this.transport.getTrialBalance(`${this.parent}/trialBalance`);
  }

  close(): void {
    this.transport.close();
  }
}
