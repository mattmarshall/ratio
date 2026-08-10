//! One copy of each distinct string, shared by everything that names it.
//!
//! # ⛔ Why this exists, measured rather than assumed
//!
//! A generated fund at four thousand lots a security holds a million open tax
//! lots across five hundred instruments. Every posting cloned its instrument
//! name to build a map key and every lot retained its acquisition date as a
//! `String`, so folding that journal allocated tens of millions of times and
//! retained a million small objects. The cold build went from 91.7 s to 335.2 s
//! and the MARK — five hundred price lookups, a number that did not change —
//! went from 395 µs to 5.0 ms. The lookups did not get slower; the heap
//! underneath them got shredded.
//!
//! The strings are five currency codes and five hundred tickers, repeated
//! across seven million entries. There is no reason for more than one copy.
//!
//! # ⛔ `Arc<str>`, NOT A `u32` SYMBOL, AND THE REASON IS ORDERING
//!
//! The obvious interner hands out an integer id and compares ids. That is
//! faster and it is WRONG HERE: an id orders by INSERTION, and
//! `Projection.positions.held` and `LotBook.open` are `BTreeMap`s whose
//! iteration order reaches a reconciliation walk, a position list and a break
//! report. Swapping lexicographic order for arrival order would reorder all of
//! them — quietly, with every existing test still green, because no test
//! asserts an order it did not itself create.
//!
//! `Arc<str>` keeps `Ord`, `Eq` and `Hash` exactly as `String` had them, and
//! serializes as a plain string. What changes is only that a clone is a
//! refcount bump rather than an allocation, and that N copies become one.
//!
//! ⚠ THE COMPARISON IS STILL A STRING COMPARISON. This buys allocation, not
//! comparison. A `u32` symbol would buy both — and the day someone wants that,
//! the thing to change is the map, not the ordering: key by an id and keep a
//! separate sorted index for anything that iterates.

use std::collections::HashSet;
use std::sync::Arc;

/// A shared string. Cloning it is a refcount bump.
pub type Text = Arc<str>;

/// Hands out one `Arc<str>` per distinct string it is shown.
///
/// ⚠ NOT GLOBAL, AND DELIBERATELY. A process-wide interner would be shared
/// mutable state on the hot path of everything, and it would keep every string
/// any book ever mentioned alive for the life of the process. One per
/// projection means the table dies with the thing that needed it.
#[derive(Clone, Debug, Default)]
pub struct Interner {
    seen: HashSet<Text>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    /// The shared copy of `s`, interning it if this is the first sight.
    ///
    /// ⚠ THE MISS PATH ALLOCATES TWICE — once to look up and once to store —
    /// and that is the right trade: a miss happens once per distinct string
    /// (five hundred times for a fund's instruments) and a hit happens once per
    /// posting (fourteen million times). Optimizing the miss would be
    /// optimizing the case that does not run.
    pub fn intern(&mut self, s: &str) -> Text {
        if let Some(found) = self.seen.get(s) {
            return Arc::clone(found);
        }
        let t: Text = Arc::from(s);
        self.seen.insert(Arc::clone(&t));
        t
    }

    /// How many distinct strings this holds. For tests and for reporting.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_string_is_stored_once_and_shared() {
        let mut i = Interner::new();
        let a = i.intern("VTI");
        let b = i.intern("VTI");
        assert_eq!(i.len(), 1, "one distinct string");
        // ⭐ THE POINT: the same allocation, not an equal one.
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(Arc::strong_count(&a), 3, "a, b, and the table");
    }

    #[test]
    fn ordering_is_lexicographic_and_not_insertion_order() {
        // ⛔ THE PROPERTY A `u32` SYMBOL WOULD HAVE BROKEN, and the reason this
        // interns to `Arc<str>` instead. `BTreeMap`s keyed on these reach a
        // reconciliation walk, a position list and a break report; ordering by
        // arrival would reorder every one of them with no test going red,
        // because no test asserts an order it did not itself create.
        let mut i = Interner::new();
        let z = i.intern("ZZZ");
        let a = i.intern("AAA");
        assert!(a < z, "interned later, still sorts first");

        let mut m = std::collections::BTreeMap::new();
        m.insert(i.intern("VOO"), 1);
        m.insert(i.intern("BND"), 2);
        m.insert(i.intern("AAA"), 3);
        assert_eq!(
            m.keys().map(|k| &**k).collect::<Vec<_>>(),
            vec!["AAA", "BND", "VOO"]
        );
    }

    #[test]
    fn an_interned_string_still_behaves_like_a_string() {
        let mut i = Interner::new();
        let t = i.intern("BRK-B");
        assert_eq!(&*t, "BRK-B");
        assert_eq!(t.len(), 5);
        // Equality is by VALUE, so a key interned by one table still finds a
        // row put there by another.
        let mut j = Interner::new();
        assert_eq!(j.intern("BRK-B"), t);
    }
}
