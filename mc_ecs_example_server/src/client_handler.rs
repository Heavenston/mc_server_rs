use mc_networking::client::{
    client_event::{ClientEvent, LoginStartResult},
    Client,
};

use std::sync::{Mutex, Arc};
use legion::{system, systems::CommandBuffer};
use rayon::prelude::*;

pub type ClientList = Mutex<Vec<HandledClient>>;

pub struct HandledClient {
    pub client: Client,
    pub event_receiver: flume::Receiver<ClientEvent>,
}

#[system]
pub fn handle_clients(
    _cmd: &mut CommandBuffer,
    #[state] client_list: &Arc<ClientList>
) {
    let mut client_list = client_list.lock().unwrap();
    client_list.par_iter_mut().for_each(|handled_client| {
        while let Ok(event) = handled_client.event_receiver.try_recv() {
            handle_client_event(handled_client, event);
        }
    });
}

pub fn handle_client_event(_handled_client: &mut HandledClient, event: ClientEvent) {
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

        _ => (),
    }
}
