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

struct SpliceBuffer {
    data: [u8; BUFFER_SIZE],
    start: usize, // first readable byte
    end: usize, // first writable byte
    parked_not_empty: Option<task::Task>,
    parked_not_full: Option<task::Task>,
}

impl SpliceBuffer {
    fn new() -> SpliceBuffer {
        SpliceBuffer {
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

    fn write<W: AsyncWrite>(&mut self, dest: &mut W) -> Poll<usize, io::Error> {
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

        Ok(Async::Ready(wsize))
    }

    fn read<R: AsyncRead>(&mut self, src: &mut R) -> Poll<usize, io::Error> {
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

        Ok(Async::Ready(rsize))
    }
}

/// A future that can splice data from one IO object to another
pub struct Splicer<R, W> {
    r: R,
    w: W,
    buf: SpliceBuffer,
    eof_r: bool,
    eof_w: bool,
}

impl<R, W> Splicer<R, W> {
    /// Creates a new DualSplicer that will splice data from a into b, and from b into a
    pub fn new(from: R, to: W) -> Splicer<R, W> {
        Splicer {
            r: from,
            w: to,
            buf: SpliceBuffer::new(),
            eof_r: false,
            eof_w: false,
        }
    }
}

impl<R, W> Future for Splicer<R, W>
    where R: AsyncRead,
          W: AsyncWrite,
{
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        if !self.eof_r {
            if let Async::Ready(n) = try!(self.buf.read(&mut self.r)) {
                if n == 0 {
                    debug!("splicer encountered EOF");
                    self.eof_r = true;
                } else {
                    task::park().unpark();
                }
            }
        }

        if !self.eof_w {
            if let Async::Ready(_) = try!(self.buf.write(&mut self.w)) {
                task::park().unpark();
            }

            if self.eof_r && self.buf.is_empty() {
                debug!("splicer sent all data");
                self.eof_w = true;
            }
        }

        if self.eof_r && self.eof_w {
            debug!("splicer finishing");
            self.w.shutdown()
        } else {
            Ok(Async::NotReady)
        }
    }
}
