use std::ops;

pub struct Bytes<'a> {
    pub slice: &'a [u8],
    pub pos: usize,
}

impl<'a> Bytes<'a> {
    #[inline]
    pub fn new(slice: &'a [u8]) -> Self {
        Self {
            slice: slice,
            pos: 0,
        }
    }

    #[inline]
    pub fn peek(&self) -> Option<u8> {
        self.slice.get(self.pos).cloned()
    }

    #[inline]
    pub unsafe fn bump(&mut self) {
        self.pos += 1;
    }
}

impl<'a> AsRef<[u8]> for Bytes<'a> {
    fn as_ref(&self) -> &'a [u8] {
        self.slice
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.pos < self.slice.len() {
            let b = unsafe { *self.slice.get_unchecked(self.pos) };
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }
}

impl<'a, Idx> ops::Index<Idx> for Bytes<'a>
where
    Idx: std::slice::SliceIndex<[u8]>,
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.as_ref()[index]
    }
}

macro_rules! complete {
    ($e:expr) => {
        match $e? {
            Status::Complete(v) => v,
            Status::Partial => return Ok(Status::Partial),
        }
    };
}

macro_rules! next {
    ($bytes:expr) => {
        match $bytes.next() {
            Some(v) => v,
            None => return Ok(Status::Partial),
        }
    };
}

macro_rules! expect {
    ($bytes:ident.next() == $pat:pat => $ret:expr) => {
        expect!(next!($bytes) => $pat => $ret)
    };
    ($e:expr => $pat:pat => $ret:expr) => {
        match $e {
            v @ $pat => v,
            _ => return $ret
        }
    };
}
