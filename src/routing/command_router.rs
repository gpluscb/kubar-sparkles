use crate::command_model_layer::{CommandModelLayer, CommandModelServiceError};
use crate::routing::InteractionRouterService;
use crate::state::StateLayer;
use std::task::{Context, Poll};
use tower::util::BoxCloneService;
use tower::{Layer, Service, ServiceExt};
use twilight_interactions::command::CommandModel;
use twilight_model::application::interaction::Interaction;
use twilight_model::id::marker::InteractionMarker;
use twilight_model::id::Id;

type BoxCommandService<Response, Error> = BoxCloneService<Interaction, Response, Error>;

#[derive(Clone, Debug)]
pub struct CommandRouterService<State, Layer, Service, BeforeStateLayer> {
    state: State,
    layer: Layer,
    inner: InteractionRouterService<Service, BeforeStateLayer>,
}

impl<State, TLayer, TService, BeforeStateLayer> Service<Interaction>
    for CommandRouterService<State, TLayer, TService, BeforeStateLayer>
where
    State: Clone + 'static,
    TService: Service<Interaction> + Clone + Send + 'static,
    TService::Response: Send + 'static,
    TService::Error: Send + 'static,
    TService::Future: Send,
{
    type Response = Option<TService::Response>;
    type Error = TService::Error;
    type Future =
        <InteractionRouterService<TService, BeforeStateLayer> as Service<Interaction>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Interaction) -> Self::Future {
        self.inner.call(req)
    }
}

impl<State, Response, Error>
    CommandRouterService<State, (), BoxCommandService<Response, Error>, ()>
{
    #[must_use]
    pub fn new(state: State) -> Self {
        CommandRouterService {
            state,
            layer: (),
            inner: InteractionRouterService::new(),
        }
    }
}

impl<State, TLayer, TService, BeforeStateLayer>
    CommandRouterService<State, TLayer, TService, BeforeStateLayer>
{
    #[must_use]
    pub fn with_layers(
        state: State,
        after_state_layer: TLayer,
        before_state_layer: BeforeStateLayer,
    ) -> Self {
        CommandRouterService {
            state,
            layer: after_state_layer,
            inner: InteractionRouterService::with_layer(before_state_layer),
        }
    }

    #[must_use]
    pub fn route<RouteService, TCommandModel>(
        mut self,
        id: Id<InteractionMarker>,
        service: RouteService,
    ) -> Self
    where
        State: Clone + Send + 'static,
        TLayer: Layer<RouteService>,
        TLayer::Service: Service<(State, TCommandModel)> + Clone + Send + 'static,
        <TLayer::Service as Service<(State, TCommandModel)>>::Future: Send,
        <TLayer::Service as Service<(State, TCommandModel)>>::Response: 'static,
        <TLayer::Service as Service<(State, TCommandModel)>>::Error: 'static,
        TCommandModel: CommandModel + Send + 'static,
        TService: Service<Interaction>,
        BeforeStateLayer: Layer<
            BoxCommandService<
                <TLayer::Service as Service<(State, TCommandModel)>>::Response,
                CommandModelServiceError<
                    <TLayer::Service as Service<(State, TCommandModel)>>::Error,
                >,
            >,
            Service = TService,
        >,
    {
        self.mut_route(id, service);
        self
    }

    pub fn mut_route<RouteService, TCommandModel>(
        &mut self,
        id: Id<InteractionMarker>,
        service: RouteService,
    ) -> Option<TService>
    where
        State: Clone + Send + 'static,
        TLayer: Layer<RouteService>,
        TLayer::Service: Service<(State, TCommandModel)> + Clone + Send + 'static,
        <TLayer::Service as Service<(State, TCommandModel)>>::Future: Send,
        <TLayer::Service as Service<(State, TCommandModel)>>::Response: 'static,
        <TLayer::Service as Service<(State, TCommandModel)>>::Error: 'static,
        TCommandModel: CommandModel + Send + 'static,
        TService: Service<Interaction>,
        BeforeStateLayer: Layer<
            BoxCommandService<
                <TLayer::Service as Service<(State, TCommandModel)>>::Response,
                CommandModelServiceError<
                    <TLayer::Service as Service<(State, TCommandModel)>>::Error,
                >,
            >,
            Service = TService,
        >,
    {
        let layered = (
            CommandModelLayer::new(),
            StateLayer::new(self.state.clone()),
            &self.layer,
        )
            .layer(service)
            .boxed_clone();

        self.inner.mut_route(id, layered)
    }

    #[must_use]
    pub fn layer<NewBeforeStateLayer>(
        self,
        new_layer: NewBeforeStateLayer,
    ) -> CommandRouterService<
        State,
        TLayer,
        NewBeforeStateLayer::Service,
        (NewBeforeStateLayer, BeforeStateLayer),
    >
    where
        NewBeforeStateLayer: Layer<TService>,
    {
        CommandRouterService {
            state: self.state,
            layer: self.layer,
            inner: self.inner.layer(new_layer),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::command_model_layer::CommandModelServiceError;
    use crate::routing::command_router::CommandRouterService;
    use crate::routing::command_service::command_service;
    use crate::test_utils;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;
    use tower::util::{MapRequestLayer, MapResponseLayer};
    use tower::{service_fn, Service, ServiceExt};
    use twilight_interactions::command::CommandModel;
    use twilight_model::id::Id;

    #[derive(CommandModel)]
    struct HasCommandModelA {}
    #[derive(CommandModel)]
    struct HasCommandModelB {}

    #[tokio::test]
    async fn command_service_test() {
        async fn command(_state: (), _model: HasCommandModelA) -> Result<i64, ()> {
            Ok(1)
        }

        async fn command2(_state: (), _model: HasCommandModelB) -> Result<i64, ()> {
            Ok(2)
        }

        let mut router = CommandRouterService::new(())
            .route(Id::new(1), command_service(command))
            .route(Id::new(2), command_service(command2));

        let res1 = router
            .ready()
            .await
            .unwrap()
            .call(test_utils::interaction(Id::new(1)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(res1, 1);

        let res2 = router
            .ready()
            .await
            .unwrap()
            .call(test_utils::interaction(Id::new(2)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(res2, 2);
    }

    #[tokio::test]
    async fn test_layers() {
        struct Mapped<S>(S);

        let atomic = Arc::new(AtomicI64::new(0));

        let mut router =
            CommandRouterService::with_layers(atomic.clone(), MapRequestLayer::new(Mapped), MapResponseLayer::new(Mapped))
                .route(
                    Id::new(1),
                    service_fn(
                        |Mapped((state, _interaction)): Mapped<(
                            Arc<AtomicI64>,
                            HasCommandModelA,
                        )>| async move {
                            state.fetch_add(1, Ordering::Relaxed);
                            Ok::<_, CommandModelServiceError<()>>(1)
                        },
                    ),
                )
                .route(
                    Id::new(2),
                    service_fn(
                        |Mapped((state, _interaction)): Mapped<(
                            Arc<AtomicI64>,
                            HasCommandModelA,
                        )>| async move {
                            state.fetch_add(1, Ordering::Relaxed);
                            Ok(2)
                        },
                    ),
                )
                .route(
                    Id::new(3),
                    service_fn(
                        |Mapped((state, _interaction)): Mapped<(
                            Arc<AtomicI64>,
                            HasCommandModelA,
                        )>| async move {
                            state.fetch_add(1, Ordering::Relaxed);
                            Ok(3)
                        },
                    ),
                );

        assert_eq!(atomic.load(Ordering::Relaxed), 0);

        let Mapped(res1) = router
            .ready()
            .await
            .unwrap()
            .call(test_utils::interaction(Id::new(1)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(res1, 1);
        assert_eq!(atomic.load(Ordering::Relaxed), 1);

        let Mapped(res2) = router
            .ready()
            .await
            .unwrap()
            .call(test_utils::interaction(Id::new(2)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(res2, 2);
        assert_eq!(atomic.load(Ordering::Relaxed), 2);

        let Mapped(res3) = router
            .ready()
            .await
            .unwrap()
            .call(test_utils::interaction(Id::new(3)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(res3, 3);
        assert_eq!(atomic.load(Ordering::Relaxed), 3);
    }
}
