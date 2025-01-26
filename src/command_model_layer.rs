use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};
use twilight_interactions::command::CommandModel;
use twilight_model::application::interaction::{Interaction, InteractionData};

// TODO: manually impl rest of derive traits
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CommandModelLayer<CommandModel> {
    phantom_data: PhantomData<CommandModel>,
}

// Manually implement derive traits because CommandModel generic param should have no bearing on
// implementations
impl<CommandModel> Clone for CommandModelLayer<CommandModel> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<CommandModel> Copy for CommandModelLayer<CommandModel> {}

impl<TService, TCommandModel> Layer<TService> for CommandModelLayer<TCommandModel> {
    type Service = CommandModelLayerService<TService, TCommandModel>;

    fn layer(&self, inner: TService) -> Self::Service {
        CommandModelLayerService {
            inner,
            phantom_data: PhantomData,
        }
    }
}

impl<CommandModel> Default for CommandModelLayer<CommandModel> {
    fn default() -> Self {
        CommandModelLayer {
            phantom_data: PhantomData,
        }
    }
}

impl<CommandModel> CommandModelLayer<CommandModel> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, PartialEq, Debug, thiserror::Error)]
pub enum CommandModelServiceError<ServiceError> {
    #[error("Error parsing command data")]
    Parse(#[from] twilight_interactions::error::ParseError),
    #[error("Interaction was not a command")]
    NotACommand,
    #[error("Inner service error")]
    Service(ServiceError),
}

// TODO: manually impl rest of derive traits
#[derive(Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct CommandModelLayerService<Service, CommandModel> {
    inner: Service,
    phantom_data: PhantomData<CommandModel>,
}

// Manually implement derive traits because CommandModel generic param should have no bearing on
// implementations
impl<Service: Clone, CommandModel> Clone for CommandModelLayerService<Service, CommandModel> {
    fn clone(&self) -> Self {
        CommandModelLayerService {
            inner: self.inner.clone(),
            phantom_data: PhantomData,
        }
    }
}

impl<Service: Copy, CommandModel> Copy for CommandModelLayerService<Service, CommandModel> {}

impl<TService, TCommandModel> Service<Interaction>
    for CommandModelLayerService<TService, TCommandModel>
where
    TService: Service<TCommandModel> + Clone + Send + 'static,
    TService::Response: 'static,
    TService::Error: 'static,
    TService::Future: Send,
    TCommandModel: CommandModel,
{
    type Response = TService::Response;
    type Error = CommandModelServiceError<TService::Error>;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner
            .poll_ready(cx)
            .map_err(CommandModelServiceError::Service)
    }

    fn call(&mut self, req: Interaction) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let command_model = TCommandModel::from_interaction(match req.data {
                Some(InteractionData::ApplicationCommand(command_data)) => (*command_data).into(),
                _ => return Err(CommandModelServiceError::NotACommand),
            })
            .map_err(CommandModelServiceError::Parse)?;

            inner
                .call(command_model)
                .await
                .map_err(CommandModelServiceError::Service)
        })
    }
}
