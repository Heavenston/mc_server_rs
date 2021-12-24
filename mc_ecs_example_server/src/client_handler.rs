use mc_networking::client::{
    client_event::{ClientEvent, LoginStartResult},
    Client,
};

use legion::{Entity, IntoQuery, system, systems::CommandBuffer, world::SubWorld};
use rayon::prelude::*;

pub struct ClientComponent {
    pub client: Client,
    pub event_receiver: flume::Receiver<ClientEvent>,
}

#[system]
#[write_component(ClientComponent)]
pub fn handle_clients(world: &mut SubWorld, cmd: &mut CommandBuffer) {
    // TODO: Find a way to avoid allocation
    let to_remove = <(Entity, &mut ClientComponent)>::query()
        .par_iter_mut(world)
        .filter_map(|(entity, client_component,)| {
            while let Ok(event) = client_component.event_receiver.try_recv() {
                if handle_client_event(client_component, event) {
                    return Some(entity);
                }
            }
            None
        })
        .copied()
        .collect::<Vec<Entity>>();
    for entity in to_remove {
        cmd.remove(entity);
    }
}

fn handle_client_event(client_component: &mut ClientComponent, event: ClientEvent) -> bool {
    let mut should_delete = false;
    match event {
        ClientEvent::ServerListPing { response } => {
            response
                .send(serde_json::from_str(include_str!("slp_response.json")).unwrap())
                .unwrap();
        }

        ClientEvent::LoginStart { username, response } => {
            response
                .send(LoginStartResult::Disconnect {
                    reason: format!(
                        "Sorry {}, you're not cool enough to join the server",
                        username
                    ),
                })
                .unwrap();
        }

        ClientEvent::Logout => {
            should_delete = true;
        }

        _ => (),
    }
    should_delete
}
