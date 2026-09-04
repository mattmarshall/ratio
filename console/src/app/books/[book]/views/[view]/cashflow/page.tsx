import Link from "next/link";
import { notFound } from "next/navigation";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { caller } from "@/lib/caller";
import { periodLabel, previousMonth, utcMonth, utcYear } from "@/lib/dates";
import {
  cashFlowStatement,
  cashForecast,
  cashShown,
  operatingCashFlowStatement,
} from "@/lib/cashflow";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Period cash-flow statement for one Personal or Operating book of record.
 *
 * ⭐ NOT A SECOND LEDGER. Beginning, ending, and the operating / investing /
 * financing lines are the trial balance in the period the chips name, under
 * `filter=cashflow-YYYY[-MM]`. Personal's bridge explains why net worth
 * moved; this page is where cash went. Operating has no bridge — the
 * sheet shows cash as a balance, the income statement shows accrual
 * profit, and this page classifies the movement.
 *
 * ⛔ A ZERO CASH ON AN EMPTY JOURNAL IS A FAKE. Beginning stays unset when
 * every account's beginning balance is 0. Ending stays unset when nothing
 * dated has landed on or before the window end. Spending down to zero is
 * a real zero — some account moved.
 *
 * ⛔ UNCLASSIFIED MOVEMENT IS A NAMED LINE. A liability the book never
 * named as a loan is not absorbed into financing. Investing stays unset
 * on Operating: chart_for(Operating) has no PPE / securities account.
 * Asset purchases stay unset on Personal: chart_for(Personal) has no
 * purchase account distinct from a cash↔investments transfer.
 *
 * Fund, project, and investment books 404 rather than wearing this label.
 *
 * Personal books also cite a cash forecast from `filter=forecast-YYYY[-MM]`:
 * posted `scheduled_*` / `forecast_*` journal kinds only. Unset when none
 * exist — not a fake zero. Envelopes, payroll, and bank predictors stay
 * refused or Connect. Operating does not wear the forecast cite.
 */
async function CashFlow({
  params,
  searchParams,
}: {
  params: Promise<{ book: string; view: string }>;
  searchParams: Promise<{ period?: string }>;
}) {
  const { book, view } = await params;
  const month = utcMonth();
  const year = utcYear();
  const last = previousMonth(month);
  const { period = month } = await searchParams;
  const window = period || month;
  const c = await caller();
  const b = await getBook(c, book);
  const personal = b.kind === "PERSONAL";
  const operating = b.kind === "OPERATING";
  if (!personal && !operating) notFound();

  const { accounts } = await listAccounts(c, book, view, "cashflow", window);
  const forecast = personal
    ? cashForecast(
        (await listAccounts(c, book, view, "forecast", window)).accounts,
      )
    : null;
  const r = operating
    ? operatingCashFlowStatement(accounts)
    : cashFlowStatement(accounts, b.loans ?? []);
  const bothUnset = r.beginning === null && r.ending === null;
  const unclassifiedMoved = r.unclassified.some((l) => l.cash !== 0n);
  const showUnclassified =
    unclassifiedMoved || (r.residual !== null && r.residual !== 0n);

  const filters: readonly Filter[] = [
    { key: month, label: periodLabel(month) },
    { key: last, label: periodLabel(last) },
    { key: year, label: year },
  ];

  const tieNote =
    r.residual === null
      ? r.beginning === null
        ? "change stays unset until both cuts exist"
        : "as-of this window's last day"
      : r.residual === 0n
        ? operating
          ? "beginning plus operating plus financing"
          : "beginning plus operating plus investing plus financing"
        : "does not tie — residual is not absorbed";

  return (
    <>
      <FilterChips
        filters={filters}
        active={window}
        param="period"
        label="Period"
        note={
          personal
            ? `${periodLabel(window)} — dated actuals; forecast cites scheduled / forecast kinds only`
            : `${periodLabel(window)} — dated entries only, not a forecast`
        }
      />

      {bothUnset ? (
        <div className="empty">
          Beginning and ending stay unset — not a measured zero cash. No dated
          journal prefix supports this period.
        </div>
      ) : null}

      <div className="tb" role="table" aria-label="Cash flow">
        <div className="posgroup">
          <div className="posacct">Cash</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Beginning
              <span className="at">
                {r.beginning === null
                  ? "no dated prefix before this window"
                  : "as-of the day before this window, dated entries only"}
              </span>
            </span>
            <span role="cell" className="num">
              {cashShown(r.beginning)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Ending
              <small>{tieNote}</small>
            </span>
            <span role="cell" className="num">
              {cashShown(r.ending)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Change
              <span className="at">
                {operating
                  ? "Δ cash this window — not period profit"
                  : "Δ cash this window — not ΔNW"}
              </span>
            </span>
            <span role="cell" className="num">
              {cashShown(r.delta)}
            </span>
          </div>
        </div>

        <div className="posgroup">
          <div className="posacct">Operating</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              {operating ? "Revenue" : "Income"}
              <span className="at">
                {operating
                  ? "period income-statement receipts, not since inception"
                  : "period P&L receipts, not since inception"}
              </span>
            </span>
            <span role="cell" className="num">
              {cashShown(r.income)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Expenses
              <span className="at">
                {operating
                  ? "operating expenses this window — a bill is not a cash outflow"
                  : "living, tax, and loan interest this window"}
              </span>
            </span>
            <span role="cell" className="num">
              {cashShown(r.expense)}
            </span>
          </div>
          {operating ? (
            <>
              <div className="tbrow static" role="row">
                <span role="cell">
                  Accounts receivable
                  <span className="at">
                    {r.receivables === null
                      ? "no Accounts receivable account on this chart"
                      : "working capital — an invoice is not a cash inflow"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(r.receivables)}
                </span>
              </div>
              <div className="tbrow static" role="row">
                <span role="cell">
                  Accounts payable
                  <span className="at">
                    {r.payables === null
                      ? "no Accounts payable account on this chart"
                      : "working capital — a vendor bill is not a cash outflow"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(r.payables)}
                </span>
              </div>
            </>
          ) : (
            <div className="tbrow static" role="row">
              <span role="cell">
                Credit cards
                <span className="at">
                  {r.creditCards === null
                    ? "no Credit cards account on this chart"
                    : "working capital — a charge is not a cash outflow"}
                </span>
              </span>
              <span role="cell" className="num">
                {cashShown(r.creditCards)}
              </span>
            </div>
          )}
          <div className="tbfoot static" role="row">
            <span role="cell">Operating</span>
            <span role="cell" className="num">
              {cashShown(r.operating)}
            </span>
          </div>
        </div>

        <div className="posgroup">
          <div className="posacct">Investing</div>
          {operating ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                PPE and securities
                <span className="at">
                  no investing account on this chart — chart_for(Operating)
                  writes cash, AR, AP, revenue, expense, owner equity
                </span>
              </span>
              <span role="cell" className="num">
                {cashShown(r.investing)}
              </span>
            </div>
          ) : (
            <>
              <div className="tbrow static" role="row">
                <span role="cell">
                  Transfers
                  <span className="at">
                    {r.transfers === null
                      ? "no Investments account on this chart"
                      : "Investments activity — the same account the net-worth bridge names as a transfer"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(r.transfers)}
                </span>
              </div>
              <div className="tbrow static" role="row">
                <span role="cell">
                  Asset purchases
                  <span className="at">
                    no purchase account distinct from a transfer
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(r.assetPurchases)}
                </span>
              </div>
            </>
          )}
          <div className="tbfoot static" role="row">
            <span role="cell">Investing</span>
            <span role="cell" className="num">
              {cashShown(r.investing)}
            </span>
          </div>
        </div>

        <div className="posgroup">
          <div className="posacct">Financing</div>
          {operating ? (
            <div className="tbrow static" role="row">
              <span role="cell">
                Owner equity
                <span className="at">
                  {r.equity === null
                    ? "no Owner equity account on this chart"
                    : "owner contribution or draw this window — not revenue"}
                </span>
              </span>
              <span role="cell" className="num">
                {cashShown(r.equity)}
              </span>
            </div>
          ) : (
            <>
              <div className="tbrow static" role="row">
                <span role="cell">
                  Principal paid on loans
                  <span className="at">
                    {r.principalPaid === null
                      ? "no [personal.loan] on the configuration in force"
                      : "named-loan liability down and cash down — the same plug /loans cites"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(
                    r.principalPaid === null ? null : -r.principalPaid,
                  )}
                </span>
              </div>
              <div className="tbrow static" role="row">
                <span role="cell">
                  Loan draws
                  <span className="at">
                    {r.drawn === null
                      ? "no [personal.loan] on the configuration in force"
                      : "named-loan credits this window — origination and further draws"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(r.drawn)}
                </span>
              </div>
              {r.equity !== null && r.equity !== 0n ? (
                <div className="tbrow static" role="row">
                  <span role="cell">
                    Opening equity
                    <span className="at">
                      household in or out this window — not income
                    </span>
                  </span>
                  <span role="cell" className="num">
                    {cashShown(r.equity)}
                  </span>
                </div>
              ) : null}
            </>
          )}
          <div className="tbfoot static" role="row">
            <span role="cell">Financing</span>
            <span role="cell" className="num">
              {cashShown(r.financing)}
            </span>
          </div>
        </div>

        {showUnclassified ? (
          <div className="posgroup">
            <div className="posacct">Unclassified</div>
            {r.unclassified.map((line) => (
              <div className="tbrow static" role="row" key={line.dimension}>
                <span role="cell">
                  <Link
                    href={`/books/${book}/views/${view}/accounts/${encodeURIComponent(line.dimension)}`}
                  >
                    {line.displayName}
                  </Link>
                  <span className="at">
                    {operating
                      ? "not revenue, expense, AR, AP, or owner equity — open the account"
                      : "not a named loan and not a transfer — open the account"}
                  </span>
                </span>
                <span role="cell" className="num">
                  {cashShown(line.cash)}
                </span>
              </div>
            ))}
            <div className="tbfoot static" role="row">
              <span role="cell">
                Residual
                <small>
                  {r.residual === null
                    ? "unset until both cash cuts exist"
                    : r.residual === 0n
                      ? "classified movement ties"
                      : "does not tie — residual is not absorbed"}
                </small>
              </span>
              <span role="cell" className="num">
                {cashShown(r.residual)}
              </span>
            </div>
          </div>
        ) : null}
      </div>

      {personal && forecast ? (
        <div className="tb" role="table" aria-label="Cash forecast">
          <div className="posgroup">
            <div className="posacct">Forecast</div>
            <div className="tbrow static" role="row">
              <span role="cell">
                Scheduled net cash
                <span className="at">
                  {forecast.net === null
                    ? "no scheduled or forecast journal in this window — unset, not a measured zero"
                    : "posted scheduled_* / forecast_* entries this window — not a bank predictor, not envelopes, not payroll"}
                </span>
              </span>
              <span role="cell" className="num">
                {cashShown(forecast.net)}
              </span>
            </div>
          </div>
        </div>
      ) : null}

      <p className="note">
        Inflow is positive, outflow is negative — cash, not{" "}
        {operating ? "accrual profit" : "net worth"}.
        {" · "}
        {personal ? (
          <>
            <Link
              href={`/books/${book}/views/${view}/bridge?period=${encodeURIComponent(window)}`}
            >
              Net-worth bridge
            </Link>
            {" · "}
          </>
        ) : null}
        <Link
          href={`/books/${book}/views/${view}/sheet?period=${encodeURIComponent(window)}`}
        >
          Balance sheet
        </Link>
        {" · "}
        <Link
          href={`/books/${book}/views/${view}/pnl?period=${encodeURIComponent(window)}`}
        >
          {operating ? "Income statement" : "Period P&L"}
        </Link>
        {personal ? (
          <>
            {" · "}
            <Link
              href={`/books/${book}/views/${view}/loans?period=${encodeURIComponent(window)}`}
            >
              Loan schedule
            </Link>
            {" · "}
            <Link href={`/books/${book}/transfer`}>Transfer</Link>
          </>
        ) : null}
      </p>
    </>
  );
}

export default withRefusal(CashFlow);
