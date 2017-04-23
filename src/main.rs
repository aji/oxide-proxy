extern crate bytes;

extern crate futures;

#[macro_use]
extern crate log;

#[macro_use]
extern crate tokio_core;

extern crate tokio_io;

mod splice;

use std::io;

use futures::Future;
use futures::Poll;
use futures::Stream;

use splice::DualSplicer;

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
            Ok(result) => Ok(result),
            Err(e) => {
                println!("an IO task errored: {}", e);
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

    let mut core = Core::new().expect("could not create tokio reactor");
    let handle = core.handle();

    let addr = "127.0.0.1:6667".parse().unwrap();
    let listener = TcpListener::bind(&addr, &handle).unwrap();

    let prev: RefCell<Option<TcpStream>> = RefCell::new(None);

    let server = listener.incoming().for_each(|(sock, _)| {
        let mut p = prev.borrow_mut();

        if let Some(p) = p.take() {
            handle.spawn(IoTask::new(DualSplicer::new(p, sock)));
        } else {
            *p = Some(sock);
        }

        Ok(())
    });

    core.run(server).expect("core exited");
}
