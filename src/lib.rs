#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]

pub mod command_model_layer;
pub mod routing;
pub mod state;

#[cfg(test)]
mod test_utils {
    use twilight_model::application::command::CommandType;
    use twilight_model::application::interaction::application_command::CommandData;
    use twilight_model::application::interaction::{Interaction, InteractionData, InteractionType};
    use twilight_model::id::marker::InteractionMarker;
    use twilight_model::id::Id;
    use twilight_model::oauth::ApplicationIntegrationMap;

    pub fn interaction(id: Id<InteractionMarker>) -> Interaction {
        Interaction {
            app_permissions: None,
            application_id: Id::new(1),
            authorizing_integration_owners: ApplicationIntegrationMap {
                guild: None,
                user: None,
            },
            channel: None,
            channel_id: None,
            context: None,
            data: Some(InteractionData::ApplicationCommand(Box::new(CommandData {
                guild_id: None,
                id: id.cast(),
                name: String::new(),
                kind: CommandType::ChatInput,
                options: vec![],
                resolved: None,
                target_id: None,
            }))),
            entitlements: vec![],
            guild: None,
            guild_id: None,
            guild_locale: None,
            id,
            kind: InteractionType::ApplicationCommand,
            locale: None,
            member: None,
            message: None,
            token: String::new(),
            user: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::routing::InteractionRouterService;
    use crate::state::{StateLayer, StateLayerService};
    use crate::test_utils;
    use tower::layer::layer_fn;
    use tower::{service_fn, ServiceExt};
    use twilight_model::id::Id;

    #[tokio::test]
    async fn it_works() {
        let value = InteractionRouterService::with_layer(StateLayer::new(2))
            .route(
                Id::new(12),
                service_fn(|(state, _interaction)| async move {
                    assert_eq!(state, 2);
                    Ok::<_, ()>(1)
                }),
            )
            .layer(layer_fn(|service: StateLayerService<_, _>| {
                service_fn(move |x| async move {
                    assert_eq!(service.oneshot(x).await.unwrap(), 1);
                    Ok::<_, ()>(42)
                })
            }))
            .oneshot(test_utils::interaction(Id::new(12)))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(value, 42);
    }
}
