use std::cmp::Ordering;
use std::iter::Peekable;

/// A sorted iterator over two independent sorted iterators, this will skip any duplicate if both iterator contain some and always prioritize the left iterator
pub struct MergeIter<L, R, T>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
{
    left: Peekable<L>,
    right: Peekable<R>,
    cmp_function: fn(&T, &T) -> Ordering,
}

impl<L, R, T> From<(L, R)> for MergeIter<L, R, T>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
    T: Ord,
{
    fn from((left, right): (L, R)) -> Self {
        Self::new(left, right)
    }
}

impl<L, R, T> MergeIter<L, R, T>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
    T: Ord,
{
    pub fn new<IL, IR>(left: IL, right: IR) -> Self
    where
        IL: IntoIterator<IntoIter = L, Item = T>,
        IR: IntoIterator<IntoIter = R, Item = T>,
    {
        Self {
            left: left.into_iter().peekable(),
            right: right.into_iter().peekable(),
            cmp_function: T::cmp,
        }
    }
}

impl<L, R, T> MergeIter<L, R, T>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
{
    pub fn with_custom_ordering<IL, IR>(left: IL, right: IR, cmp: fn(&T, &T) -> Ordering) -> Self
    where
        IL: IntoIterator<IntoIter = L, Item = T>,
        IR: IntoIterator<IntoIter = R, Item = T>,
    {
        Self {
            left: left.into_iter().peekable(),
            right: right.into_iter().peekable(),
            cmp_function: cmp,
        }
    }
}

impl<L, R, T> Iterator for MergeIter<L, R, T>
where
    L: Iterator<Item = T>,
    R: Iterator<Item = T>,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        // Temporary enum to prevent issues with the borrow checker
        enum Next {
            Left,
            Right,
            Both, // in case of equality, take left and discard right
            None,
        }
        let n = match (self.left.peek(), self.right.peek()) {
            (Some(ref l), Some(ref r)) => match (self.cmp_function)(l, r) {
                Ordering::Greater => Next::Right,
                Ordering::Less => Next::Left,
                Ordering::Equal => Next::Both,
            },
            (Some(_), None) => Next::Left,
            (None, Some(_)) => Next::Right,
            (None, None) => Next::None,
        };
        match n {
            Next::Left => self.left.next(),
            Next::Right => self.right.next(),
            Next::Both => {
                let _ = self.right.next();
                self.left.next()
            }
            Next::None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (l, lo) = self.left.size_hint();
        let (r, ro) = self.right.size_hint();
        (
            l + r,
            match (lo, ro) {
                (Some(lo), Some(ro)) => Some(lo + ro),
                // no predictable upper bound
                _ => None,
            },
        )
    }
}

#[test]
fn merge_test_no_dup() {
    let a = [1, 4, 5, 7, 8];
    let b = [2, 3, 6, 9];
    let sorted: Vec<_> = MergeIter::new(a, b).collect();
    assert!(sorted.is_sorted());
}

#[test]
fn merge_test_with_dup() {
    let a = [1, 2, 4, 5, 7, 8];
    let b = [2, 3, 4, 6, 9];
    let mut sorted: Vec<_> = MergeIter::new(a, b).collect();
    assert!(sorted.is_sorted());
    let len = sorted.len();
    sorted.dedup();
    assert_eq!(sorted.len(), len);
}
