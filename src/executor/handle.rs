use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct JoinHandle<Out> {
    output: Option<Out>,
}

impl<Out> Future for JoinHandle<Out> {
    type Output = Out;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        todo!()
    }
}
