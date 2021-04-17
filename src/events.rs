use std::time::{Duration, Instant};

use flume::{Receiver, Selector, Sender};

/// simple generic event queue
pub struct EventQueue<E> {
    sender: EventSender<E>,
    recv: Receiver<E>,
    immediate_recv: Receiver<E>,
}

impl<E> Default for EventQueue<E>
where
    E: Send + 'static,
{
    /// create new event queue
    fn default() -> Self {
        let (sender, recv) = flume::unbounded();
        let (immediate_sender, immediate_recv) = flume::unbounded();
        let (timer_sender, timer_receiver) = flume::unbounded();

        let sender = EventSender::new(sender, immediate_sender, timer_sender);

        Self {
            immediate_recv,
            recv,
            sender,
        }
    }
}

impl<E> EventQueue<E>
where
    E: Send + 'static,
{
    pub fn sender(&self) -> &EventSender<E> { &self.sender }

    // blocking receiver
    pub fn recv(&mut self) -> E {
        if !self.immediate_recv.is_empty() {
            self.immediate_recv.recv().unwrap()
        } else {
            Selector::new()
                .recv(&self.recv, |v| v.unwrap())
                .recv(&self.immediate_recv, |v| v.unwrap())
                .wait()
        }
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> Option<E> {
        if !self.immediate_recv.is_empty() {
            self.immediate_recv.recv().ok()
        } else {
            Selector::new()
                .recv(&self.recv, |v| v.unwrap())
                .recv(&self.immediate_recv, |v| v.unwrap())
                .wait_timeout(timeout)
                .ok()
        }
    }

    // non-blocking
    pub fn try_recv(&mut self) -> Option<E> {
        self.immediate_recv
            .try_recv()
            .or_else(|_| self.recv.try_recv())
            .ok()
    }
}

#[derive(Debug)]
pub struct EventSender<E> {
    tx: Sender<E>,
    tx_immediate: Sender<E>,
    tx_timer: Sender<(Instant, E)>
}

impl<T> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        EventSender {
            tx: self.tx.clone(),
            tx_immediate: self.tx_immediate.clone(),
            tx_timer: self.tx_timer.clone(),
        }
    }
}

impl<E> EventSender<E> {
    fn new(inner: Sender<E>, immediate: Sender<E>, tx_timer: Sender<(Instant, E)>) -> Self { Self { tx: inner, tx_immediate: immediate , tx_timer} }

    pub fn send(&self, value: E) { self.tx.send(value); }

    pub fn send_immediate(&self, value: E) { self.tx.send(value); }

    pub fn send_after(&self, value: E, after: Duration) { self.;}

    pub fn inner(&self) -> &Sender<E> { &self.tx }
}
