use crate::{AtomicDiff, Diffable};

impl<'a> Diffable<'a> for uuid::Uuid {
    type Diff = AtomicDiff<'a, Self>;

    fn diff(&self, other: &'a Self) -> Self::Diff {
        if self == other {
            AtomicDiff::Unchanged
        } else {
            AtomicDiff::Replaced(&other)
        }
    }
}
