//! A `Send` one-shot signal: wgpu's completion callbacks must be `Send`, JS functions are not, so the
//! callback fires this and an async block awaits it (then resolves a JS Promise).

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Default)]
struct State {
    fired: bool,
    waker: Option<Waker>,
}

#[derive(Clone, Default)]
pub struct Signal(Arc<Mutex<State>>);

impl Signal {
    pub fn fire(&self) {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        s.fired = true;
        if let Some(w) = s.waker.take() {
            w.wake();
        }
    }
}

impl Future for Signal {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut s = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if s.fired {
            Poll::Ready(())
        } else {
            s.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Wake;

    struct Flag(Mutex<bool>);
    impl Wake for Flag {
        fn wake(self: Arc<Self>) {
            *self.0.lock().unwrap() = true;
        }
    }

    #[test]
    fn pending_until_fired_then_ready_and_woken() {
        let flag = Arc::new(Flag(Mutex::new(false)));
        let waker = Waker::from(flag.clone());
        let mut cx = Context::from_waker(&waker);
        let mut s = Signal::default();
        let other = s.clone();
        assert!(Pin::new(&mut s).poll(&mut cx).is_pending());
        other.fire();
        assert!(*flag.0.lock().unwrap());
        assert!(Pin::new(&mut s).poll(&mut cx).is_ready());
    }
}
