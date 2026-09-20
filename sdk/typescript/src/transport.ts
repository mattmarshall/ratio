import type { Account, ListAccountsResponse, ListTransactionsResponse, Posting, Transaction, TrialBalance } from "./types.js";

/** What a transport does: the seven calls, on typed messages. Both transports implement this. */
export interface Transport {
  getTransaction(name: string): Promise<Transaction>;
  listTransactions(parent: string, pageSize: number, pageToken: string): Promise<ListTransactionsResponse>;
  createTransaction(parent: string, postings: Posting[], transactionId: string, controlPlaneHash: string): Promise<Transaction>;
  getAccount(name: string): Promise<Account>;
  listAccounts(parent: string, pageSize: number, pageToken: string): Promise<ListAccountsResponse>;
  createAccount(parent: string, account: Account, accountId: string): Promise<Account>;
  getTrialBalance(name: string): Promise<TrialBalance>;
  close(): void;
}
