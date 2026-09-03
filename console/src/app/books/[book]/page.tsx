import Link from "next/link";
import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { SCREEN_GROUPS, screenHref, screensFor } from "@/lib/screens";
import { KIND_SHORT } from "@/lib/templates";
import { getBook, getView } from "@/wire/client";

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
 * capital first, then the ABOR warehouse (#70). The hub is how you open
 * the citable figures after CreateBook.
 */
export default async function BookPage({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book } = await params;
  const c = await caller();
  const b = await or404(getBook(c, book));
  const view = b.defaultView
    ? await or404(getView(c, book, b.defaultView))
    : null;
  const personal = b.kind === "PERSONAL";
  const project = b.kind === "PROJECT";
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
      <dl className="kv">
        <dt>Entries</dt>
        <dd className="num">{count(b.entryCount)}</dd>
        <dt>Trial balance</dt>
        <dd className="num">{b.trialBalanceDifference}</dd>
        <dt>Configuration</dt>
        <dd>{b.configDigest || "none"}</dd>
        <dt>Default view</dt>
        <dd>{b.defaultView || "—"}</dd>
        {view && !project ? (
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
              {b.budget ? money(b.budget) : "unset — [project] budget on the configuration"}
            </dd>
          </>
        ) : null}
        {personal ? (
          <>
            <dt>Budget</dt>
            <dd className="num">
              {b.budget
                ? money(b.budget)
                : "unset — [personal] budget on the configuration"}
            </dd>
          </>
        ) : null}
      </dl>
      <nav className="places places-hub" aria-label="Places">
        {SCREEN_GROUPS.map((g) => (
          <div key={g.id} className="placegroup">
            <span className="placehead">{g.label}</span>
            {places.filter((s) => s.group === g.id).map((s) => {
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
        ))}
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
          <Link href={`/books/${book}/record`}>Record a cost, bill, retainage hold, or capitalize WIP</Link>
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
