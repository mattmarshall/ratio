import Link from "next/link";
import { Unavailable } from "@/components/Unavailable";
import { bookOf, viewOf } from "@/lib/data";
import { isoDate } from "@/lib/dates";
import { basisOf, count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { screenHref, screensFor } from "@/lib/screens";

export const dynamic = "force-dynamic";

/**
 * A book of record — the view itself, not a child screen under it.
 *
 * ⭐ THIS IS THE PAGE #53 ASKED FOR. `funds/{fund}/views/{view}` is a real
 * resource (the AIP name; the URL is `/books/{book}/views/{view}` after #64)
 * and used to land on the exceptions queue because the segment had a layout
 * and no page. The URL is now the citation.
 *
 * ⚠ THE STAT TILES IN THE LAYOUT ARE CHROME ON EVERY CHILD. They are
 * restated here because this page is what you send, and because a render test
 * mounts the page without the layout. `viewOf` is the one GetView both share.
 */
export default async function ViewPage({
  params,
}: {
  params: Promise<{ book: string; view: string }>;
}) {
  const { book, view } = await params;
  const viewRead = await or404(viewOf(book, view));
  if (viewRead.unavailable !== null) {
    return <Unavailable why={viewRead.unavailable} />;
  }
  const bookRead = await or404(bookOf(book));
  if (bookRead.unavailable !== null) {
    return <Unavailable why={bookRead.unavailable} />;
  }
  const v = viewRead.value;
  const b = bookRead.value;
  const basis = basisOf(v.basis, v.settlementOpenDays);
  const personal = b.kind === "PERSONAL";
  const project = b.kind === "PROJECT";
  const operating = b.kind === "OPERATING";
  const places = screensFor(b.kind).filter((s) => s.scoped);
  const tb = (BigInt(v.totalDebit) - BigInt(v.totalCredit)).toString();

  return (
    <section className="lots">
      <div className="loghead">
        <span>{v.displayName}</span>
        <span className="sortnote">{basis}</span>
      </div>
      <dl className="kv">
        <dt>
          {personal
            ? "Net worth"
            : project || operating
              ? "Assets less liabilities"
              : "Net asset value"}
        </dt>
        <dd className="num">{money(v.netAssetValue)}</dd>
        {personal ? null : project || operating ? (
          <>
            <dt>Trial balance</dt>
            <dd className="num">{money(tb)}</dd>
          </>
        ) : (
          <>
            <dt>Open difference</dt>
            <dd className="num">{money(v.openDifference)}</dd>
            <dt>Open breaks</dt>
            <dd className="num">{count(v.openBreakCount)}</dd>
            <dt>Unplaceable</dt>
            <dd className="num">{count(v.unplaceableEntryCount)}</dd>
          </>
        )}
        {personal || operating ? (
          <>
            <dt>Unplaceable</dt>
            <dd className="num">{count(v.unplaceableEntryCount)}</dd>
          </>
        ) : null}
        <dt>Basis</dt>
        <dd>
          {basis}
          {v.calendar ? <span className="sub"> {v.calendar}</span> : null}
        </dd>
        <dt>Recognised through</dt>
        <dd>{isoDate(v.recognisedThrough)}</dd>
        <dt>Declared</dt>
        <dd>
          {v.declared
            ? "yes — a term of the configuration"
            : "no — the journal's own order"}
        </dd>
      </dl>
      <p className="note">
        {places.map((s, i) => (
          <span key={s.segment}>
            {i ? " · " : null}
            <Link href={screenHref(book, view, s, "books")}>{s.label}</Link>
          </span>
        ))}
        {" · "}
        <Link href={`/books/${book}`}>Book</Link>
      </p>
    </section>
  );
}
