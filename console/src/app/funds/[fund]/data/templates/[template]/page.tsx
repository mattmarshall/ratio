import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { getTemplate } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One mapping template, as it will read a row. */
export default async function TemplateDetail({
  params,
}: {
  params: Promise<{ fund: string; template: string }>;
}) {
  const { fund, template } = await params;
  const c = await caller();
  const t = await or404(getTemplate(c, fund, template));

  return (
    <aside className="detail" aria-label="Template">
      <div className="dhead">
        <h2>{t.templateId}</h2>
        <div className="sub">{t.factKind}</div>
      </div>
      <div className="dsec">
        <h3>What it maps</h3>
        <p className="prov">{t.form || "—"}</p>
      </div>
      <div className="dsec">
        <h3>What reading it does</h3>
        <p className="note">
          {t.posts
            ? "A row read through this template posts to the journal."
            : "A row read through this template records a fact. Something else posts it, and until then it sits in the pending queue."}
        </p>
        <p className="note">Fund {fund}.</p>
      </div>
    </aside>
  );
}
