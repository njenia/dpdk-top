//! Fixed-size ring buffer for time-series history.

use std::mem::MaybeUninit;

/// Fixed-capacity ring buffer. No heap allocation after creation.
pub struct RingBuffer<T, const N: usize> {
    buf: [MaybeUninit<T>; N],
    head: usize,
    len: usize,
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> RingBuffer<T, N> {
    pub fn new() -> Self {
        Self {
            buf: std::array::from_fn(|_| MaybeUninit::uninit()),
            head: 0,
            len: 0,
        }
    }

    /// Push a value; oldest is dropped if full.
    pub fn push(&mut self, value: T) {
        if N == 0 {
            return;
        }
        let idx = if self.len < N {
            self.len += 1;
            self.len - 1
        } else {
            // Overwrite oldest
            unsafe { self.buf[self.head].assume_init_drop() }
            self.head = (self.head + 1) % N;
            (self.head + N - 1) % N
        };
        self.buf[idx].write(value);
    }

    /// Number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Capacity.
    pub fn capacity(&self) -> usize {
        N
    }

    /// Iterate from oldest to newest.
    pub fn iter(&self) -> RingBufferIter<'_, T, N> {
        RingBufferIter { ring: self, pos: 0 }
    }

    /// Get slice of the last n values (newest last). Allocates a Vec.
    pub fn last_n(&self, n: usize) -> Vec<&T> {
        let n = n.min(self.len);
        if n == 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(n);
        let start = if self.len < N {
            0
        } else {
            (self.head + self.len - n) % N
        };
        for i in 0..n {
            let idx = (start + i) % N;
            out.push(unsafe { self.buf[idx].assume_init_ref() });
        }
        out
    }
}

impl<T: Copy, const N: usize> RingBuffer<T, N> {
    /// Copy last n values into a slice (newest last). Caller must ensure slice length.
    pub fn copy_last_n(&self, out: &mut [T]) {
        let n = out.len().min(self.len);
        if n == 0 {
            return;
        }
        let start = if self.len < N {
            0
        } else {
            (self.head + self.len - n) % N
        };
        for (i, slot) in out.iter_mut().take(n).enumerate() {
            let idx = (start + i) % N;
            *slot = unsafe { *self.buf[idx].assume_init_ref() };
        }
    }
}

impl<T, const N: usize> Drop for RingBuffer<T, N> {
    fn drop(&mut self) {
        for i in 0..self.len {
            let idx = (self.head + i) % N;
            unsafe { self.buf[idx].assume_init_drop() }
        }
    }
}

pub struct RingBufferIter<'a, T, const N: usize> {
    ring: &'a RingBuffer<T, N>,
    pos: usize,
}

impl<'a, T, const N: usize> Iterator for RingBufferIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.ring.len {
            return None;
        }
        let idx = if N == 0 {
            return None;
        } else if self.ring.len < N {
            self.pos
        } else {
            (self.ring.head + self.pos) % N
        };
        self.pos += 1;
        Some(unsafe { self.ring.buf[idx].assume_init_ref() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_basic() {
        let mut r: RingBuffer<u32, 4> = RingBuffer::new();
        r.push(1);
        r.push(2);
        r.push(3);
        let v: Vec<u32> = r.iter().copied().collect();
        assert_eq!(v, [1, 2, 3]);
        r.push(4);
        r.push(5);
        let v: Vec<u32> = r.iter().copied().collect();
        assert_eq!(v, [2, 3, 4, 5]);
    }
}
