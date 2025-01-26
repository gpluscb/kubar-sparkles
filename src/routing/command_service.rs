use std::future::Future;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CommandService<F> {
    function: F,
}

impl<F, TState, TCommandModel, Fut, Response, Error> Service<(TState, TCommandModel)>
    for CommandService<F>
where
    F: FnMut(TState, TCommandModel) -> Fut,
    Fut: Future<Output = Result<Response, Error>>,
{
    type Response = Response;
    type Error = Error;
    type Future = Fut;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, (state, model): (TState, TCommandModel)) -> Self::Future {
        (self.function)(state, model)
    }
}

#[must_use]
pub fn command_service<F>(function: F) -> CommandService<F> {
    CommandService { function }
}
