import Link from "next/link";
import { Unavailable } from "@/components/Unavailable";
import { bookRecord, viewOf } from "@/lib/data";
import { count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { SCREEN_GROUPS, screenHref, screensFor } from "@/lib/screens";
import { KIND_SHORT } from "@/lib/templates";

export const dynamic = "force-dynamic";

/**
 * The book of record as its own page — see #53.
 *
 * A fund is an optional filing. This screen does not send a personal or
 * project book through `/funds/{fund}/…` to be opened.
 *
 * ⭐ KIND SELECTS THE PLACES. A personal book that listed Exceptions / NAV
 * would be a fake label on fund-ops screens (#65, #83). A project book
 * that listed them is the same defect (#66, #85). An investment book cites
 * capital first, then the ABOR warehouse (#70). An operating book cites
 * a balance sheet, period income statement, period cash-flow, and
 * AR/AP aging, not Fund NAV and not Project `/billing` (#108, #118,
 * #117). The hub is how you open the citable figures after CreateBook.
 *
 * ⛔ BARE `or404(getBook)` WAS THE #253 HOLE THE LAYOUT ALREADY CLOSED.
 * The identity layout wraps GetBook in `orTransient` and returns
 * `<Unavailable>` without `{children}`. The page still runs. A held
 * session the gateway refuses became `Refused(401)` from `orAuth` and
 * left this server component — Next redacts that to `#441`. Same wrap
 * as the view page: `bookRecord` / `viewOf`, then 404 for a missing book.
 */
export default async function BookPage({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book } = await params;
  const bookRead = await or404(bookRecord(book));
  if (bookRead.unavailable !== null) {
    return <Unavailable why={bookRead.unavailable} />;
  }
  const b = bookRead.value;
  const viewRead = b.defaultView
    ? await or404(viewOf(book, b.defaultView))
    : null;
  if (viewRead && viewRead.unavailable !== null) {
    return <Unavailable why={viewRead.unavailable} />;
  }
  const view = viewRead?.value ?? null;
  const personal = b.kind === "PERSONAL";
  const project = b.kind === "PROJECT";
  const operating = b.kind === "OPERATING";
  const places = screensFor(b.kind);

  return (
    <>
      <div className="qhead">
        <h1>{b.displayName}</h1>
        <div className="subhead">
          <span>{KIND_SHORT[b.kind] ?? b.kind}</span>
          {b.fund ? <span>filed as {b.fund}</span> : <span>independent</span>}
          {b.organization ? <span>org {b.organization}</span> : null}
        </div>
      </div>
      <dl className="kv hubfacts">
        <dt>Entries</dt>
        <dd className="num">{count(b.entryCount)}</dd>
        <dt>Trial balance</dt>
        <dd className="num">{b.trialBalanceDifference}</dd>
        <dt>Configuration</dt>
        <dd>{b.configDigest || "none"}</dd>
        <dt>Default view</dt>
        <dd>{b.defaultView || "—"}</dd>
        {view && !project && !operating ? (
          <>
            <dt>
              {personal ? "Net worth" : "NAV"}, in {b.defaultView}
            </dt>
            <dd className="num">{money(view.netAssetValue)}</dd>
          </>
        ) : null}
        {project ? (
          <>
            <dt>Budget</dt>
            <dd className="num">
              {b.budget ? (
                money(b.budget)
              ) : (
                <>
                  unset —{" "}
                  <Link href={`/books/${book}/config`}>set one on Configuration</Link>
                </>
              )}
            </dd>
          </>
        ) : null}
        {personal ? (
          <>
            <dt>Budget</dt>
            <dd className="num">
              {b.budget ? (
                money(b.budget)
              ) : (
                <>
                  unset —{" "}
                  <Link href={`/books/${book}/config`}>set one on Configuration</Link>
                </>
              )}
            </dd>
          </>
        ) : null}
      </dl>
      <nav className="places places-hub" aria-label="Places">
        {SCREEN_GROUPS.map((g) => {
          const items = places.filter((s) => s.group === g.id);
          if (!items.length) return null;
          return (
            <div key={g.id} className="placegroup">
              <span className="placehead">{g.label}</span>
              {items.map((s) => {
                const href =
                  s.scoped && !b.defaultView
                    ? undefined
                    : screenHref(book, b.defaultView, s, "books");
                return href ? (
                  <Link key={s.segment} href={href}>
                    {s.label}
                  </Link>
                ) : (
                  <span key={s.segment} className="placeoff">
                    {s.label}
                    <small>needs a book of record</small>
                  </span>
                );
              })}
            </div>
          );
        })}
      </nav>
      {personal && b.defaultView ? (
        <p className="note">
          <Link href={`/books/${book}/transfer`}>Transfer between accounts</Link>
          {" · "}
          <Link href={`/books/${book}/record`}>Record income or an expense</Link>
        </p>
      ) : null}
      {project && b.defaultView ? (
        <p className="note">
          <Link href={`/books/${book}/record`}>Record a cost, bill, collection, retainage hold, change order, or capitalize WIP</Link>
        </p>
      ) : null}
      {operating && b.defaultView ? (
        <p className="note">
          <Link href={`/books/${book}/record`}>Record a sale, invoice, collection, bill, or expense</Link>
          {" · "}
          control balances on the sheet; aged open items by due date
        </p>
      ) : null}
      {b.fund ? (
        <p className="note">
          <Link href={`/${b.fund}`}>Fund filing</Link>
        </p>
      ) : null}
    </>
  );
}
