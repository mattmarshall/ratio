import Link from "next/link";
import { FilterChips, type Filter } from "@/components/FilterChips";
import { isMoneyBreak } from "@/lib/breaks";
import { caller } from "@/lib/caller";
import { money, SEVERITY_CLASS } from "@/lib/format";
import { listBreaks } from "@/wire/client";

export const dynamic = "force-dynamic";

const FILTERS: readonly Filter[] = [
  { key: "", label: "All" },
  { key: "blocking", label: "Blocking" },
  { key: "unexplained", label: "Unexplained" },
];

/** The exceptions queue, ordered by money at risk. */
export default async function Breaks({
  params,
  searchParams,
}: {
  params: Promise<{ fund: string }>;
  searchParams: Promise<{ filter?: string }>;
}) {
  const { fund } = await params;
  const { filter = "" } = await searchParams;
  const c = await caller();
  const { breaks } = await listBreaks(c, fund, filter || undefined);

  return (
    <>
      <FilterChips
        filters={FILTERS}
        active={filter}
        label="Filter breaks"
        note="Ordered by money at risk"
      />

      <ul className="rows">
        {breaks.length === 0 ? (
          <li>
            <div className="empty">
              {filter
                ? "Nothing matches this filter."
                : "No breaks — this period has not been reconciled yet."}
            </div>
          </li>
        ) : null}
        {breaks.map((b) => {
          const id = b.name.split("/").pop()!;
          return (
            <li key={b.name}>
              {/* ⭐ THE CITATION. This URL is what an operator sends a colleague,
                  and the reason this console exists as more than one page. */}
              <Link className="row" href={`/funds/${fund}/breaks/${id}`}>
                <span className={`sev ${SEVERITY_CLASS[b.severity]}`} />
                <span>
                  <div className="title">
                    {b.account}
                    {isMoneyBreak(b) ? ` — ${money(b.difference)}` : ""}
                  </div>
                  <div className="why">
                    {b.cause}
                    {isMoneyBreak(b)
                      ? ` · ${b.postings.length} posting${
                          b.postings.length === 1 ? "" : "s"
                        } under configuration ${b.configDigest.slice(0, 7)}`
                      : ""}
                  </div>
                </span>
                <span className={`amt num${b.explained ? "" : " pos"}`}>
                  {isMoneyBreak(b) ? money(b.difference) : "—"}
                  {/* Three readings, not two. A break somebody explained and a
                      break whose explanation has been overtaken by a later
                      figure are both "not open", and telling a reader they are
                      the same thing is how the second one gets closed without
                      anybody looking at what moved. */}
                  <small>
                    {b.explained
                      ? "explained"
                      : b.explanation
                        ? "stale"
                        : "open"}
                  </small>
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </>
  );
}
