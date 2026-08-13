/**
 * ⚠ THIS IS ALSO WHAT A FUND YOU MAY NOT SEE LOOKS LIKE, AND THAT IS ON
 * PURPOSE. `Console::book_path` refuses a fund a caller has no grant for with
 * the same error as one that does not exist, so the console cannot be used to
 * enumerate other people's funds. Saying "you do not have access to this" here
 * would undo the boundary by describing it.
 */
export default function NotFound() {
  return (
    <div className="empty">
      <p>No such thing here.</p>
      <p className="p2">
        Either it does not exist, or it is not yours to read — this console does
        not say which.
      </p>
    </div>
  );
}
