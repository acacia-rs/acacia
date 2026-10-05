use std::fmt;
use std::marker::PhantomData;

use bytes::{BufMut, Bytes, BytesMut};

use crate::skip::skip_checking_utf8;
use crate::{Flavor, Nbt, Result, read, write};

/// A root tag kept as its wire bytes: reading one only finds its end, and nothing is built until
/// [`Raw::decode`] (README.md, "Performance").
#[derive(Clone)]
pub struct Raw<F> {
    bytes: Storage,
    flavor: PhantomData<F>,
}

/// Longest document kept inline. An item registry holds one tiny document per item, so allocating
/// for each was a quarter of decoding it.
const INLINE: usize = 38;

#[derive(Clone)]
enum Storage {
    Inline { len: u8, bytes: [u8; INLINE] },
    Heap(Bytes),
}

impl Storage {
    fn new(doc: &[u8]) -> Self {
        if doc.len() > INLINE {
            return Storage::Heap(Bytes::copy_from_slice(doc));
        }
        let mut bytes = [0; INLINE];
        bytes[..doc.len()].copy_from_slice(doc);
        Storage::Inline { len: doc.len() as u8, bytes }
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            Storage::Inline { len, bytes } => &bytes[..usize::from(*len)],
            Storage::Heap(bytes) => bytes,
        }
    }
}

const END: &[u8] = &[0];

impl<F: Flavor> Raw<F> {
    pub fn read(r: &mut &[u8]) -> Result<Self> {
        Self::read_lossy(r).map(|(raw, _)| raw)
    }

    /// Like [`Raw::read`]; the flag is [`crate::read_lossy`]'s, known without decoding.
    pub fn read_lossy(r: &mut &[u8]) -> Result<(Self, bool)> {
        let start = *r;
        let lossy = skip_checking_utf8::<F>(r)?;
        let doc = &start[..start.len() - r.len()];
        Ok((Raw { bytes: Storage::new(doc), flavor: PhantomData }, lossy))
    }

    pub fn write(&self, w: &mut BytesMut) {
        w.put_slice(self.as_bytes());
    }

    /// Fails only for bytes that did not come from [`Raw::read`] or an [`Nbt`].
    pub fn decode(&self) -> Result<Nbt> {
        read::<F>(&mut self.as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub fn is_end(&self) -> bool {
        self.as_bytes() == END
    }
}

impl<F> PartialEq for Raw<F> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes.as_slice() == other.bytes.as_slice()
    }
}

impl<F> Eq for Raw<F> {}

impl<F> Default for Raw<F> {
    fn default() -> Self {
        Raw { bytes: Storage::new(END), flavor: PhantomData }
    }
}

impl<F: Flavor> From<&Nbt> for Raw<F> {
    fn from(nbt: &Nbt) -> Self {
        let mut w = BytesMut::new();
        write::<F>(&mut w, nbt);
        Raw { bytes: Storage::new(&w), flavor: PhantomData }
    }
}

impl<F: Flavor> From<Nbt> for Raw<F> {
    fn from(nbt: Nbt) -> Self {
        (&nbt).into()
    }
}

impl<F: Flavor> fmt::Debug for Raw<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.decode() {
            Ok(nbt) => nbt.fmt(f),
            Err(e) => write!(f, "Raw(invalid: {e})"),
        }
    }
}
