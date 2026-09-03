"use client";

import { useActionState, useState } from "react";
import {
  Commit,
  Derived,
  Field,
  Picker,
  Ticket,
  type Step,
} from "@/components/Ticket";
import { count, money } from "@/lib/format";
import type { Template } from "@/wire/types";
import { admit, read, type AdmitResult, type ReadResult } from "./actions";

/**
 * Read a file, having first seen what it would produce — then admit what
 * resolves.
 *
 * The same preview-then-commit shape as recording an event, and the same
 * reason: `validateOnly` runs the identical code path and records nothing, so a
 * mapping you have watched run beats one you have to imagine.
 *
 * ⛔ TWO WRITES, NOT ONE, AND THEY ARE DELIBERATELY SEPARATE. Reading a file
 * turns bytes into facts; admitting turns the facts that fully resolve into
 * journal entries. A file can be read and admit nothing — every row pending on
 * an instrument nobody has added — and collapsing the two would hide exactly
 * that state.
 */
export function IngestForm({
  fund,
  templates,
}: {
  fund: string;
  templates: Template[];
}) {
  const [readResult, readAction, reading] = useActionState<ReadResult, FormData>(
    read,
    null,
  );
  const [admitResult, admitAction, admitting] = useActionState<
    AdmitResult,
    FormData
  >(admit, null);

  const [templateId, setTemplateId] = useState("");
  const [origin, setOrigin] = useState("");
  const [content, setContent] = useState("");

  const r = readResult?.ok ? readResult.response : null;
  const a = admitResult?.ok ? admitResult.response : null;
  const template = templates.find((t) => t.templateId === templateId) ?? null;

  const now = [templateId, origin.trim(), content].join(" ");
  const previewed =
    readResult?.ok === true &&
    readResult.response.validateOnly &&
    readResult.signature === now;

  const templatePicker = (
    <Picker
      name="Template"
      value={templateId}
      onValue={setTemplateId}
      empty={templates.length === 0 ? "No template in force" : "Choose a template"}
      options={templates.map((t) => ({
        value: t.templateId,
        label: `${t.templateId} — ${t.factKind}`,
      }))}
    />
  );
  const originField = (
    <Field
      name="Origin"
      value={origin}
      onValue={setOrigin}
      hint="origin/file.csv"
    />
  );
  const contentField = (
    <Field
      name="Content"
      value={content}
      onValue={setContent}
      rows={8}
      hint="paste the file"
    />
  );

  const steps: Step[] = [
    {
      id: "template",
      label: "Template",
      ask: "Which mapping does this file arrive under?",
      why: [
        "The template says what each column means.",
        <>
          For a file that posts, it also says which rule each row falls under —
          which rule a counterparty&rsquo;s <code>B</code> means is a mapping
          decision, and belongs with the rest of the mapping.
        </>,
      ],
      answer: templateId || null,
      body: (
        <>
          {templatePicker}
          {template ? (
            <p className="ruleform">
              <b>{template.templateId}</b>
              {template.form}
              {/* ⛔ REFERENCE DATA POSTS NOTHING BY DESIGN. Reporting the design
                  as a fault is how a demo comes to show red for the thing
                  working. */}
              {template.posts
                ? " Its facts post through the rules in force."
                : " Reference data: recorded, resolved and citable, and it never touches the books."}
            </p>
          ) : null}
        </>
      ),
    },
    {
      id: "origin",
      label: "Origin",
      ask: "Where did it come from?",
      why: [
        "Carried as provenance on every fact the file produces.",
        "So a figure can name the delivery it was read out of.",
      ],
      answer: origin.trim() || null,
      body: originField,
    },
    {
      id: "content",
      label: "Content",
      ask: "The file itself.",
      why: [
        "A machine's output, so it is set in monospace.",
        "A column that does not line up is harder to check.",
      ],
      answer: content ? `${content.split("\n").length} lines` : null,
      body: contentField,
    },
    {
      id: "review",
      label: "Review",
      ask: "What this would produce.",
      why: [
        "Reading records facts and nothing more.",
        "Nothing reaches the journal until they are admitted, which is the step below.",
      ],
      answer: null,
      body: r ? (
        <>
          <Derived k="Rows" v={count(r.rowCount)} from="read out of the file" />
          <Derived
            k="Ready to post"
            v={count(r.readyCount)}
            from={`of ${count(r.factCount)} facts, ${count(r.newFactCount)} new`}
          />
          {/* ⛔ PER ROW, NEVER PER FILE. A read that reports only its successes
              looks identical to one that dropped half the file. */}
          {r.rejected.length ? (
            <Derived
              k="Rejected"
              v={String(r.rejected.length)}
              bad
              from="rows the template could not map"
            />
          ) : null}
        </>
      ) : (
        <p className="why">Preview to see what the template makes of it.</p>
      ),
    },
  ];

  return (
    <>
      <Ticket
        title="Read a file"
        summary={
          templateId && content
            ? `Read ${content.split("\n").length} lines under ${templateId}.`
            : "A template, a file, and what the mapping makes of it before any of it is kept."
        }
        steps={steps}
        compact={
          <div className="form">
            {templatePicker}
            {originField}
            {contentField}
          </div>
        }
        actions={
          <form action={readAction}>
            <input type="hidden" name="fund" value={fund} />
            <input type="hidden" name="templateId" value={templateId} />
            <input type="hidden" name="origin" value={origin} />
            <input type="hidden" name="content" value={content} />
            <Commit
              preview="Preview"
              commit="Read"
              previewed={previewed}
              pending={reading}
              ready={Boolean(templateId) && Boolean(content)}
            />
          </form>
        }
      >
        {readResult && !readResult.ok ? (
          <div className="empty err">{readResult.error}</div>
        ) : null}

        {r ? (
          <div className="prev">
            <div className="ph">
              <span>
                {r.validateOnly ? "What this would produce" : "What it produced"}
              </span>
              <span>{r.deliveryDigest.slice(0, 12)}</span>
            </div>
            <dl className="kv">
              <dt>Rows</dt>
              <dd className="num">{count(r.rowCount)}</dd>
              <dt>Facts</dt>
              <dd className="num">{count(r.factCount)}</dd>
              <dt>New</dt>
              <dd className="num">{count(r.newFactCount)}</dd>
              <dt>Ready to post</dt>
              <dd className="num">{count(r.readyCount)}</dd>
            </dl>
            {r.rejected.length ? (
              <div className="postings">
                {r.rejected.map((row) => (
                  <div className="posting" key={`${row.row}-${row.reason}`}>
                    <span>
                      <div className="p1">row {row.row}</div>
                      <div className="p2">{row.reason}</div>
                    </span>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}
      </Ticket>

      {/* Admitting is its own write, so it gets its own bar rather than a third
          button on the one above. ⚠ It posts EVERY fact that fully resolves, not
          only the ones this file brought. */}
      <form action={admitAction}>
        <input type="hidden" name="fund" value={fund} />
        <div className="loghead">
          <span>Admit</span>
          <span className="sortnote">post every fact that fully resolves</span>
        </div>
        <div className="formbar">
          <span className="sortnote">
            The fact becomes an event and the rules already in force decide the
            entries — nothing here knows how a trade books.
          </span>
          <button className="act" type="submit" disabled={admitting}>
            {admitting ? "…" : "Preview"}
          </button>
          <button
            className="act primary"
            type="submit"
            name="commit"
            value="1"
            disabled={admitting}
          >
            Admit
          </button>
        </div>
      </form>

      {admitResult && !admitResult.ok ? (
        <div className="empty err">{admitResult.error}</div>
      ) : null}

      {a ? (
        <div className="prev">
          <div className="ph">
            <span>{a.validateOnly ? "What admitting would do" : "What it did"}</span>
          </div>
          <dl className="kv">
            <dt>Posted</dt>
            <dd className="num">{count(a.postedCount)}</dd>
            <dt>Recorded</dt>
            <dd className="num">{count(a.recordedCount)}</dd>
            <dt>Still pending</dt>
            <dd className="num">{count(a.pendingCount)}</dd>
            <dt>Net asset value</dt>
            <dd className="num">{money(a.netAssetValue)}</dd>
            <dt>Was</dt>
            <dd className="num">{money(a.previousNetAssetValue)}</dd>
          </dl>
          {a.refused.length ? (
            <ul className="note">
              {a.refused.map((x) => (
                <li key={x}>{x}</li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}
    </>
  );
}
