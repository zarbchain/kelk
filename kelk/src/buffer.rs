/// A static buffer with 16kB of capacity.
pub struct StaticBuffer {
    /// The static buffer with a total capacity of 16kB.
    buffer: [u8; Self::CAPACITY],
}

impl StaticBuffer {
    /// The capacity of the static buffer.
    const CAPACITY: usize = 1 << 14; // 16kB

    /// Creates a new static buffer.
    pub const fn new() -> Self {
        Self {
            buffer: [0; Self::CAPACITY],
        }
    }
}

impl core::ops::Index<core::ops::RangeFull> for StaticBuffer {
    type Output = [u8];

    fn index(&self, index: core::ops::RangeFull) -> &Self::Output {
        core::ops::Index::index(&self.buffer[..], index)
    }
}

impl core::ops::IndexMut<core::ops::RangeFull> for StaticBuffer {
    fn index_mut(&mut self, index: core::ops::RangeFull) -> &mut Self::Output {
        core::ops::IndexMut::index_mut(&mut self.buffer[..], index)
    }
}

/// Utility to allow for non-heap allocating encoding into a static buffer.
///
/// Required by `ScopedBuffer` internals.
struct EncodeScope<'a> {
    buffer: &'a mut [u8],
    len: usize,
}

impl<'a> From<&'a mut [u8]> for EncodeScope<'a> {
    fn from(buffer: &'a mut [u8]) -> Self {
        Self { buffer, len: 0 }
    }
}

impl<'a> EncodeScope<'a> {
    /// Returns the capacity of the encoded scope.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the length of the encoded scope.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the internal mutable byte slice.
    pub fn into_buffer(self) -> &'a mut [u8] {
        self.buffer
    }
}

impl<'a> scale::Output for EncodeScope<'a> {
    fn write(&mut self, bytes: &[u8]) {
        debug_assert!(
            self.len() + bytes.len() <= self.capacity(),
            "encode scope buffer overflowed. capacity is {} but last write index is {}",
            self.capacity(),
            self.len() + bytes.len(),
        );
        let start = self.len;
        let len_bytes = bytes.len();
        self.buffer[start..(start + len_bytes)].copy_from_slice(bytes);
        self.len += len_bytes;
    }

    fn push_byte(&mut self, byte: u8) {
        debug_assert_ne!(
            self.len(),
            self.capacity(),
            "encode scope buffer overflowed. capacity is {} and buffer is already full",
            self.capacity(),
        );
        self.buffer[self.len] = byte;
        self.len += 1;
    }
}

/// Scoped access to an underlying bytes buffer.
///
/// # Note
///
/// This is used to efficiently chunk up ink!'s internal static 16kB buffer
/// into smaller sub buffers for processing different parts of computations.
#[derive(Debug)]
pub struct ScopedBuffer<'a> {
    offset: usize,
    buffer: &'a mut [u8],
}

impl<'a> From<&'a mut [u8]> for ScopedBuffer<'a> {
    fn from(buffer: &'a mut [u8]) -> Self {
        Self { offset: 0, buffer }
    }
}

impl<'a> ScopedBuffer<'a> {
    /// Returns the first `len` bytes of the buffer as mutable slice.
    pub fn take(&mut self, len: usize) -> &'a mut [u8] {
        debug_assert_eq!(self.offset, 0);
        debug_assert!(len <= self.buffer.len());
        let len_before = self.buffer.len();
        let buffer = core::mem::take(&mut self.buffer);
        let (lhs, rhs) = buffer.split_at_mut(len);
        self.buffer = rhs;
        debug_assert_eq!(lhs.len(), len);
        let len_after = self.buffer.len();
        debug_assert_eq!(len_before - len_after, len);
        lhs
    }

    /// Encode the given value into the scoped buffer and return the sub slice
    /// containing all the encoded bytes.
    pub fn take_encoded<T>(&mut self, value: &T) -> &'a mut [u8]
    where
        T: scale::Encode,
    {
        debug_assert_eq!(self.offset, 0);
        let buffer = core::mem::take(&mut self.buffer);
        let mut encode_scope = EncodeScope::from(buffer);
        scale::Encode::encode_to(&value, &mut encode_scope);
        let encode_len = encode_scope.len();
        let _ = core::mem::replace(&mut self.buffer, encode_scope.into_buffer());
        self.take(encode_len)
    }

    /// Returns all of the remaining bytes of the buffer as mutable slice.
    pub fn take_rest(self) -> &'a mut [u8] {
        debug_assert_eq!(self.offset, 0);
        debug_assert!(!self.buffer.is_empty());
        self.buffer
    }
}