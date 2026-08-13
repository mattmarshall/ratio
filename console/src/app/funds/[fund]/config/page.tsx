import Link from "next/link";
import { caller } from "@/lib/caller";
import { listConfigVersions } from "@/wire/client";

export const dynamic = "force-dynamic";

/** Every configuration this fund has run under, newest first. */
export default async function Config({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { configVersions } = await listConfigVersions(c, fund);

  return (
    <section className="log" aria-label="Configuration">
      <div className="loghead">
        <span>Configuration</span>
        <span className="sortnote">content-addressed, newest first</span>
      </div>
      {configVersions.length === 0 ? (
        <div className="empty">This book has no approved configuration.</div>
      ) : null}
      {configVersions.map((v) => {
        const id = v.name.split("/").pop()!;
        return (
          <Link className="logrow" key={v.name} href={`/funds/${fund}/config/${id}`}>
            <span className="t num">{v.digest.slice(0, 7)}</span>
            <span className="w">
              <b>
                {v.subject || "configuration"} {v.active ? "· active" : ""}
              </b>
              <div className="cfg">
                {v.rules.length} rule{v.rules.length === 1 ? "" : "s"} · approved
                by {v.actor} · {v.approveTime.slice(0, 16).replace("T", " ")}
              </div>
            </span>
            <span className="amt num">{v.sequence}</span>
          </Link>
        );
      })}
    </section>
  );
}
