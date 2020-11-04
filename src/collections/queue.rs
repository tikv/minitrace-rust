// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

//! A queue, different from [VecDeque] in stdlib, indexes elements by
//! fixed incremental numbers, while [VecDeque] indexes elements by
//! relative offset by head element.
//!
//! [VecDeque]: std::collections::VecDeque

use std::collections::VecDeque;
use std::ops::{Index, IndexMut};

/// A queue whose elements are indexed by fixed incremental number
///
/// # Main Difference Between VecDeque and FixedIndexQueue
///
/// ```
/// # use std::collections::VecDeque;
/// # use minitrace::collections::queue::FixedIndexQueue;
/// // VecDeque
/// let mut deque = VecDeque::new();
/// deque.push_back(0);
/// deque.push_back(1);
/// assert_eq!(deque[1], 1);
///
/// deque.push_back(2);
/// deque.pop_front();
/// assert_eq!(deque[1], 2);
///
/// // FixedIndexQueue
/// let mut queue = FixedIndexQueue::new();
/// queue.push_back(0);
/// queue.push_back(1);
/// assert_eq!(queue[1], 1);
///
/// queue.push_back(2);
/// queue.pop_front();
/// assert_eq!(queue[1], 1);
/// ```
///
/// The index `1` indexes different elements in [VecDeque] according to how remaining
/// elements are arranged. For [FixedIndexQueue], an element is indexed by the same number,
/// so you can store indexes as references and won't worry about an index's misreferencing
/// to another element.
#[derive(Debug, Clone, Default)]
pub struct FixedIndexQueue<T> {
    offset: usize,
    internal: VecDeque<T>,
}

impl<T> FixedIndexQueue<T> {
    /// Creates an empty `FixedIndexQueue` begins with an initial offset `0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let queue: FixedIndexQueue<i32> = FixedIndexQueue::new();
    /// ```
    pub fn new() -> Self {
        Self {
            offset: 0,
            internal: VecDeque::new(),
        }
    }

    /// Creates an empty `FixedIndexQueue` with space for at least `capacity` elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let queue: FixedIndexQueue<i32> = FixedIndexQueue::with_capacity(1024);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            offset: 0,
            internal: VecDeque::with_capacity(capacity),
        }
    }

    /// Appends an element to the back of the `FixedIndexQueue` and
    /// returns the index of that element.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    ///
    /// assert_eq!(queue.push_back(42), 0);
    /// assert_eq!(queue.push_back(24), 1);
    /// assert_eq!(&queue[0], &42);
    /// assert_eq!(&queue[1], &24);
    /// ```
    #[inline]
    pub fn push_back(&mut self, value: T) -> usize {
        let index = self.offset.wrapping_add(self.internal.len());
        self.internal.push_back(value);
        index
    }

    /// Removes the first element and returns it, or `None` if
    /// the `FixedIndexQueue` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// queue.push_back(42);
    /// queue.push_back(24);
    ///
    /// assert_eq!(&queue[1], &24);
    /// assert_eq!(queue.pop_front(), Some(42));
    /// assert_eq!(&queue[1], &24);
    /// assert_eq!(queue.pop_front(), Some(24));
    /// assert_eq!(queue.pop_front(), None);
    /// ```
    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.internal.is_empty() {
            None
        } else {
            self.offset = self.offset.wrapping_add(1);
            self.internal.pop_front()
        }
    }

    /// Returns the index of the head element, or `None`
    /// if the `FixedIndexQueue` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// queue.push_back(42);
    /// queue.push_back(24);
    ///
    /// assert_eq!(queue.head_index(), Some(0));
    /// queue.pop_front();
    /// assert_eq!(queue.head_index(), Some(1));
    /// queue.pop_front();
    /// assert_eq!(queue.head_index(), None);
    /// ```
    #[inline]
    pub fn head_index(&self) -> Option<usize> {
        if self.internal.is_empty() {
            None
        } else {
            Some(self.offset)
        }
    }

    /// Returns the index of the last element, or `None`
    /// if the `FixedIndexQueue` is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// queue.push_back(42);
    /// assert_eq!(queue.last_index(), Some(0));
    /// queue.pop_front();
    ///
    /// queue.push_back(24);
    /// assert_eq!(queue.last_index(), Some(1));
    /// queue.pop_front();
    /// assert_eq!(queue.last_index(), None);
    /// ```
    #[inline]
    pub fn last_index(&self) -> Option<usize> {
        if self.internal.is_empty() {
            None
        } else {
            Some(
                self.offset
                    .wrapping_add(self.internal.len())
                    .wrapping_sub(1),
            )
        }
    }

    /// Returns `true` if the `FixedIndexQueue` is empty
    ///
    /// # Examples
    ///
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// assert!(queue.is_empty());
    /// queue.push_back(42);
    /// assert!(!queue.is_empty())
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.internal.is_empty()
    }

    /// Returns the number of elements in the `FixedIndexQueue`.
    ///
    /// # Examples
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// assert_eq!(queue.len(), 0);
    ///
    /// queue.push_back(42);
    /// assert_eq!(queue.len(), 1);
    ///
    /// queue.push_back(43);
    /// queue.push_back(44);
    /// assert_eq!(queue.len(), 3);
    ///
    /// queue.pop_front();
    /// queue.pop_front();
    /// assert_eq!(queue.len(), 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.internal.len()
    }

    /// Returns the index of the next new element.
    ///
    /// # Examples
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// assert_eq!(queue.next_index(), 0);
    ///
    /// queue.push_back(42);
    /// assert_eq!(queue.next_index(), 1);
    ///
    /// queue.push_back(43);
    /// queue.push_back(44);
    /// assert_eq!(queue.next_index(), 3);
    ///
    /// queue.pop_front();
    /// queue.pop_front();
    /// assert_eq!(queue.next_index(), 3);
    /// ```
    #[inline]
    pub fn next_index(&self) -> usize {
        self.offset.wrapping_add(self.internal.len())
    }

    /// Returns `true` if exists an element indexed by `index`.
    ///
    /// # Examples
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// assert!(!queue.idx_is_valid(0));
    ///
    /// queue.push_back(42);
    /// assert!(queue.idx_is_valid(0));
    /// assert!(!queue.idx_is_valid(1));
    ///
    /// queue.push_back(43);
    /// queue.push_back(44);
    /// assert!(queue.idx_is_valid(0));
    /// assert!(queue.idx_is_valid(1));
    /// assert!(queue.idx_is_valid(2));
    ///
    /// queue.pop_front();
    /// assert!(!queue.idx_is_valid(0));
    /// queue.pop_front();
    /// assert!(!queue.idx_is_valid(1));
    /// queue.pop_front();
    /// assert!(!queue.idx_is_valid(2));
    /// ```
    #[inline]
    pub fn idx_is_valid(&self, index: usize) -> bool {
        index.wrapping_sub(self.offset) < self.internal.len()
    }

    /// Clears the `FixedIndexQueue`, removing all values.
    ///
    /// # Examples
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// queue.push_back(42);
    /// queue.push_back(24);
    ///
    /// assert!(!queue.is_empty());
    /// queue.clear();
    /// assert!(queue.is_empty());
    ///
    /// assert_eq!(queue.push_back(42), 2);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.offset = self.offset.wrapping_add(self.len());
        self.internal.clear();
    }

    /// Removes values before `index`. Should make sure `index` is valid.
    ///
    /// # Examples
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// queue.push_back(42);
    /// queue.push_back(24);
    /// queue.push_back(43);
    ///
    /// queue.remove_before(2);
    /// assert_eq!(queue.len(), 1);
    /// ```
    #[inline]
    pub fn remove_before(&mut self, index: usize) {
        assert!(self.idx_is_valid(index), "index {} isn't valid", index);

        let mut count = index.wrapping_sub(self.offset);
        while count > 0 {
            self.internal.pop_front();
            count -= 1;
        }
    }

    /// Returns a front-to-end iter.
    ///
    /// # Examples
    /// ```
    /// use minitrace::collections::queue::FixedIndexQueue;
    ///
    /// let mut queue = FixedIndexQueue::new();
    /// queue.push_back(42);
    /// queue.push_back(24);
    /// queue.push_back(43);
    ///
    /// let b: &[_] = &[&42, &24, &43];
    /// let c: Vec<&i32> = queue.iter().collect();
    /// assert_eq!(&c[..], b);
    /// ```
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.internal.iter()
    }

    #[inline]
    pub fn take_queue_from(&mut self, index: usize) -> VecDeque<T> {
        let skip = index.wrapping_sub(self.offset);
        self.offset = self.offset.wrapping_add(self.internal.len());
        let mut vd = self.internal.split_off(0);
        for _ in 0..skip {
            vd.pop_front();
        }
        vd
    }
}

impl<T: Clone> FixedIndexQueue<T> {
    #[inline]
    pub fn clone_queue_from(&self, index: usize) -> VecDeque<T> {
        let mut r = self.internal.clone();
        for _ in 0..index.wrapping_sub(self.offset) {
            r.pop_front();
        }
        r
    }
}

impl<T> Index<usize> for FixedIndexQueue<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.internal[index.wrapping_sub(self.offset)]
    }
}

impl<T> IndexMut<usize> for FixedIndexQueue<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.internal[index.wrapping_sub(self.offset)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index() {
        let mut queue = FixedIndexQueue::new();

        queue.push_back(0);
        queue.push_back(1);
        queue.push_back(2);

        assert_eq!(&queue[0], &0);
        assert_eq!(&queue[1], &1);
        assert_eq!(&queue[2], &2);

        queue[0] = 1;
        assert_eq!(&queue[0], &1);

        queue.pop_front();

        assert_eq!(&queue[1], &1);
        assert_eq!(&queue[2], &2);

        queue.pop_front();
        assert_eq!(&queue[2], &2);

        queue[2] = 3;
        assert_eq!(&queue[2], &3);

        assert_eq!(queue.pop_front(), Some(3));
        assert_eq!(queue.pop_front(), None);
    }
}
