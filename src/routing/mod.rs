pub mod command_router;
pub mod command_service;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use twilight_model::application::interaction::Interaction;
use twilight_model::id::marker::InteractionMarker;
use twilight_model::id::Id;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct InteractionRouterService<Service, Layer = ()> {
    layer: Layer,
    routes: HashMap<Id<InteractionMarker>, Service>,
}

impl<TService, Layer> Default for InteractionRouterService<TService, Layer>
where
    Layer: Default,
{
    fn default() -> Self {
        InteractionRouterService {
            layer: Layer::default(),
            routes: HashMap::new(),
        }
    }
}

impl<TService, TLayer> Service<Interaction> for InteractionRouterService<TService, TLayer>
where
    TService: Service<Interaction> + Clone + Send + 'static,
    TService::Response: Send + 'static,
    TService::Error: Send + 'static,
    TService::Future: Send,
{
    type Response = Option<TService::Response>;
    type Error = TService::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.routes
            .values_mut()
            .map(|service| service.poll_ready(cx))
            .find(|elem| !matches!(elem, Poll::Ready(Ok(()))))
            .unwrap_or(Poll::Ready(Ok(())))
    }

    fn call(&mut self, interaction: Interaction) -> Self::Future {
        if let Some(service) = self.routes.get_mut(&interaction.id) {
            let clone = service.clone();
            let mut service = std::mem::replace(service, clone);

            Box::pin(async move { service.call(interaction).await.map(Some) })
        } else {
            Box::pin(std::future::ready(Ok(None)))
        }
    }
}

impl<TService> InteractionRouterService<TService> {
    #[must_use]
    pub fn new() -> Self {
        InteractionRouterService::default()
    }
}

impl<TService, TLayer> InteractionRouterService<TService, TLayer> {
    #[must_use]
    pub fn with_layer(layer: TLayer) -> Self {
        InteractionRouterService {
            layer,
            routes: HashMap::new(),
        }
    }

    #[must_use]
    pub fn route<RouteService, Request>(
        mut self,
        id: Id<InteractionMarker>,
        service: RouteService,
    ) -> Self
    where
        TLayer: Layer<RouteService, Service = TService>,
        TService: Service<Interaction>,
        RouteService: Service<Request>,
    {
        self.mut_route(id, service);
        self
    }

    pub fn mut_route<RouteService, Request>(
        &mut self,
        id: Id<InteractionMarker>,
        service: RouteService,
    ) -> Option<TService>
    where
        TLayer: Layer<RouteService, Service = TService>,
        TService: Service<Interaction>,
        RouteService: Service<Request>,
    {
        let layered = self.layer.layer(service);
        self.routes.insert(id, layered)
    }

    #[must_use]
    pub fn layer<NewLayer>(
        self,
        layer: NewLayer,
    ) -> InteractionRouterService<NewLayer::Service, (NewLayer, TLayer)>
    where
        NewLayer: Layer<TService>,
    {
        let routes = self
            .routes
            .into_iter()
            .map(|(id, service)| (id, layer.layer(service)))
            .collect();

        InteractionRouterService {
            layer: (layer, self.layer),
            routes,
        }
    }
}
