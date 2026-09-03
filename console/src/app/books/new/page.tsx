import { CreateBookForm } from "./CreateBookForm";

export const dynamic = "force-dynamic";

/** Open a journal. No fund and no organization are required. */
export default function NewBook() {
  return (
    <main className="queue">
      <div className="qhead">
        <h1>New book</h1>
        <div className="subhead">
          <span>
            Same kernel as every other book. A template chooses the chart,
            not a product fork. Nothing here files a fund or an organization.
          </span>
        </div>
      </div>
      <CreateBookForm />
    </main>
  );
}
