
use mc_networking::client::{Client, client_event::{ClientEvent, LoginStartResult}};

pub async fn handle_client(_client: Client, event_receiver: flume::Receiver<ClientEvent>) {
    while let Ok(event) = event_receiver.recv_async().await {
        match event {
            ClientEvent::ServerListPing {
                response,
            } => {
                response.send(serde_json::from_str(include_str!("slp_response.json")).unwrap()).unwrap();
            }

            ClientEvent::LoginStart {
                username, response,
            } => {
                response.send(LoginStartResult::Disconnect {
                    reason: format!("Sorry {}, you're not cool enough to join the server", username)
                }).unwrap();
                break;
            }

            _ => (),
        }
    }
}
