import Link from "next/link";
import { caller } from "@/lib/caller";
import { count } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getBook, getView } from "@/wire/client";

export const dynamic = "force-dynamic";

const KIND_LABEL: Record<string, string> = {
  PERSONAL: "Personal",
  INVESTMENT: "Investment",
  PROJECT: "Project",
  UNSPECIFIED: "Book",
};

/**
 * The book of record as its own page — see #53.
 *
 * A fund is an optional filing. This screen does not send a personal or
 * project book through `/funds/{fund}/…` to be opened.
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

  return (
    <main className="queue">
      <div className="qhead">
        <h1>{b.displayName}</h1>
        <div className="subhead">
          <span>{KIND_LABEL[b.kind] ?? b.kind}</span>
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
        {view ? (
          <>
            <dt>NAV, in {b.defaultView}</dt>
            <dd className="num">{view.netAssetValue}</dd>
          </>
        ) : null}
      </dl>
      {b.defaultView ? (
        <p className="note">
          <Link href={`/books/${book}/views/${b.defaultView}/breaks`}>
            Open the exceptions queue
          </Link>
          {b.fund ? (
            <>
              {" · "}
              <Link href={`/${b.fund}`}>Fund view</Link>
            </>
          ) : null}
        </p>
      ) : null}
    </main>
  );
}
