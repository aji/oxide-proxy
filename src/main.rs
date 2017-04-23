extern crate badlog;
extern crate bytes;
extern crate futures;
extern crate tokio_io;

#[macro_use]
extern crate log;
#[macro_use]
extern crate tokio_core;

mod splice;

use std::io;

use futures::Async;
use futures::Future;
use futures::Poll;
use futures::Stream;

use tokio_io::AsyncRead;

use splice::Splicer;

struct IoTask<T> {
    task: T
}

impl<T> IoTask<T> {
    fn new(task: T) -> IoTask<T> {
        IoTask { task: task }
    }
}

impl<T> Future for IoTask<T> where T: Future<Error=io::Error> {
    type Item = T::Item;
    type Error = ();

    fn poll(&mut self) -> Poll<T::Item, ()> {
        match self.task.poll() {
            Ok(Async::NotReady) => {
                Ok(Async::NotReady)
            },

            Ok(Async::Ready(x)) => {
                info!("an IO task finished");
                Ok(Async::Ready(x))
            },

            Err(e) => {
                warn!("an IO task errored: {}", e);
                Err(())
            }
        }
    }
}

fn main() {
    use std::cell::RefCell;

    use tokio_core::net::TcpListener;
    use tokio_core::net::TcpStream;
    use tokio_core::reactor::Core;

    badlog::init_from_env("LOG");

    let mut core = Core::new().expect("could not create tokio reactor");
    let handle = core.handle();

    let addr = "127.0.0.1:6667".parse().unwrap();
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    let prev: RefCell<Option<TcpStream>> = RefCell::new(None);

    let server = listener.incoming().for_each(|(sock, _)| {
        let mut p = prev.borrow_mut();

        if let Some(p) = p.take() {
            let (ar, aw) = p.split();
            let (br, bw) = sock.split();
            handle.spawn(IoTask::new(Splicer::new(ar, bw)));
            handle.spawn(IoTask::new(Splicer::new(br, aw)));
        } else {
            *p = Some(sock);
        }

        Ok(())
    });

    core.run(server).expect("core exited");
}
