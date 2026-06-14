#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Posting {
    pub dim: i64,
    pub amount: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    pub postings: Vec<Posting>,
}
