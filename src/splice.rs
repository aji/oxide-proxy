//! Copying data between two asynchronous IO objects

use std::io;
use std::mem;

use futures::Async;
use futures::Future;
use futures::Poll;
use futures::task;

use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;

/// Size of buffered data
const BUFFER_SIZE: usize = 4096;

struct Splice {
    data: [u8; BUFFER_SIZE],
    start: usize, // first readable byte
    end: usize, // first writable byte
    parked_not_empty: Option<task::Task>,
    parked_not_full: Option<task::Task>,
}

impl Splice {
    fn new() -> Splice {
        Splice {
            data: unsafe { mem::uninitialized() },
            start: 0,
            end: 0,
            parked_not_empty: None,
            parked_not_full: None,
        }
    }

    fn is_empty(&self) -> bool {
        // strict == would work, but defensive coding
        self.start >= self.end
    }

    fn is_full(&self) -> bool {
        // strict == would work, but defensive coding
        self.end >= BUFFER_SIZE
    }

    fn poll_not_empty(&mut self) -> Async<()> {
        if self.is_empty() {
            self.parked_not_empty = Some(task::park());
            Async::NotReady
        } else {
            Async::Ready(())
        }
    }

    fn poll_not_full(&mut self) -> Async<()> {
        if self.is_full() {
            self.parked_not_full = Some(task::park());
            Async::NotReady
        } else {
            Async::Ready(())
        }
    }

    fn write<W: AsyncWrite>(&mut self, dest: &mut W) -> Poll<(), io::Error> {
        if self.poll_not_empty().is_not_ready() {
            return Ok(Async::NotReady);
        }

        let wsize = try_nb!(dest.write(&self.data[self.start..self.end]));
        self.start += wsize;

        if self.start == self.end {
            self.start = 0;
            self.end = 0;
        }

        debug!("spliced out {} bytes, s={} e={}", wsize, self.start, self.end);
        assert!(self.start <= self.end);

        self.poll_not_empty();

        if !self.is_full() {
            self.parked_not_full.take().map(|t| t.unpark());
        }

        Ok(Async::NotReady)
    }

    fn read<R: AsyncRead>(&mut self, src: &mut R) -> Poll<(), io::Error> {
        if self.poll_not_full().is_not_ready() {
            return Ok(Async::NotReady);
        }

        let rsize = try_nb!(src.read(&mut self.data[self.end..]));
        self.end += rsize;

        debug!("spliced in {} bytes, s={} e={}", rsize, self.start, self.end);
        assert!(self.end <= BUFFER_SIZE);

        self.poll_not_full();

        if !self.is_empty() {
            self.parked_not_empty.take().map(|t| t.unpark());
        }

        Ok(Async::NotReady)
    }
}

/// A future that can splice data across two asynchronous IO objects
pub struct DualSplicer<A, B> {
    a: A,
    b: B,
    a2b: Splice,
    b2a: Splice,
}

impl<A, B> DualSplicer<A, B> {
    /// Creates a new DualSplicer that will splice data from a into b, and from b into a
    pub fn new(a: A, b: B) -> DualSplicer<A, B> {
        DualSplicer {
            a: a,
            b: b,
            a2b: Splice::new(),
            b2a: Splice::new(),
        }
    }
}

impl<A, B> Future for DualSplicer<A, B>
    where A: AsyncRead + AsyncWrite,
          B: AsyncRead + AsyncWrite
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        try!(self.a2b.read(&mut self.a));
        try!(self.b2a.read(&mut self.b));

        try!(self.a2b.write(&mut self.b));
        try!(self.b2a.write(&mut self.a));

        Ok(Async::NotReady)
    }
}
