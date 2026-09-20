# ratio-sdk (Python)

```sh
pip install ratio-sdk            # REST: no dependencies
pip install 'ratio-sdk[grpc]'    # + grpcio for the gRPC transport
```

```python
import ratio

client = ratio.Client("http://127.0.0.1:50051", book="fund-1")     # transport="grpc" for gRPC

txn = client.transactions.create([
    ratio.Posting(dim=2, amount=-125_000),      # cash out
    ratio.Posting(dim=1, amount=125_000),       # investments up
])
print(txn.name)                                 # books/fund-1/transactions/txn-0

try:
    client.transactions.create([ratio.Posting(dim=2, amount=-125_000), ratio.Posting(dim=1, amount=124_999)])
except ratio.RatioError as e:
    assert e.status == "FAILED_PRECONDITION"    # nothing reached the journal
    print(e.message)                            # the store's own words

for t in client.transactions:                   # the journal, in posting order
    ...
equity = client.accounts.create(dim=20, display_name="Capital contributions", account_type=ratio.AccountType.EQUITY)
equity.normal_side                              # Side.CREDIT — derived by theorem, never sent
client.trial_balance().difference               # 0
```

Amounts are Python `int`s and exact. Postings take an optional `currency_code`
(its own conservation law), `instrument` and `quantity`. Client-assigned ids
go through `create(..., id="trade-7")`; pinning the rules goes through
`create(..., control_plane_hash=digest)`.

The gRPC transport ships its own wire codec (`ratio/_wire.py`), so `grpcio` is
the only dependency and there are no stubs to regenerate. See
[`../README.md`](../README.md) for the API, the error model and what is checked.
