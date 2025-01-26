use std::marker::PhantomData;
use std::task::{Context, Poll};
use tower::{Layer, Service};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct StateLayer<Request, State> {
    state: State,
    phantom_data: PhantomData<Request>,
}

impl<Request, State> StateLayer<Request, State> {
    #[must_use]
    pub fn new(state: State) -> Self {
        StateLayer {
            state,
            phantom_data: PhantomData,
        }
    }
}

impl<Request, State, TService> Layer<TService> for StateLayer<Request, State>
where
    State: Clone,
    TService: Service<(State, Request)>,
{
    type Service = StateLayerService<State, TService>;

    fn layer(&self, inner: TService) -> Self::Service {
        StateLayerService {
            state: self.state.clone(),
            inner,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct StateLayerService<State, TService> {
    state: State,
    inner: TService,
}

impl<State, TService, Request, Response, Error> Service<Request>
    for StateLayerService<State, TService>
where
    State: Clone,
    TService: Service<(State, Request), Response = Response, Error = Error>,
{
    type Response = Response;
    type Error = Error;
    type Future = <TService as Service<(State, Request)>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        self.inner.call((self.state.clone(), req))
    }
}
