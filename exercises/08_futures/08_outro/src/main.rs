use outro_08::data;
use outro_08::data::{Status, Ticket};
use outro_08::store::{TicketId, TicketStore};
use ticket_fields::{TicketDescription, TicketTitle};
use tide::prelude::*;
use tide::{Body, Request, Response, StatusCode};
use tokio::net::TcpListener;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(remote = "TicketTitle")]
pub struct TicketTitleSerializer(String);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(remote = "TicketDescription")]
pub struct TicketDescriptionSerializer(String);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(remote = "Ticket")]
pub struct TicketSerializer {
    pub id: TicketId,
    #[serde(with="TicketTitleSerializer")] pub title: TicketTitle,
    #[serde(with="TicketDescriptionSerializer")] pub description: TicketDescription,
    pub status: Status,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetTicketResponse(#[serde(with="TicketSerializer")]Ticket);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTicketResponse {
    pub ticket_id: TicketId,
}

impl TryInto<data::TicketDraft> for CreateTicketRequest {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<data::TicketDraft, Self::Error> {
        let title = self.title.try_into()?;
        let description = self.description.try_into()?;
        let result = data::TicketDraft { title, description };
        Ok(result)
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = listen(Some(8080)).await?;
    run_server(listener).await
}

async fn listen(port: Option<u16>) -> std::io::Result<TcpListener> {
    let bind_addr = format!("127.0.0.1:{}", port.unwrap_or(0));
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    println!("Server listening on {}", local_addr);
    Ok(listener)
}

async fn run_server(listener: TcpListener) -> std::io::Result<()> {
    let store = TicketStore::new();
    let mut app = tide::with_state(store);
    app.at("/tickets").post(new_ticket);
    app.at("/tickets/:id").get(get_ticket);
    // Use the listener for the Tide app
    app.listen(listener.into_std()?).await
}

async fn new_ticket(mut req: Request<TicketStore>) -> tide::Result {
    // TODO improve error handling
    let ticket_request: CreateTicketRequest = req.body_json().await?;
    let store = req.state();
    let id: TicketId = store.write().await.add_ticket(ticket_request.try_into()?);

    let response_body = CreateTicketResponse { ticket_id: id };

    let mut response = Response::new(StatusCode::Ok);
    response.set_body(Body::from_json(&response_body)?);
    Ok(response)
}

async fn get_ticket(req: Request<TicketStore>) -> tide::Result {
    // TODO improve error handling
    let ticket_id: u64 = req.param("id")?.parse::<u64>()?;
    let store = req.state();
    let inner_lock = store.read().await.get(TicketId(ticket_id)).unwrap();
    let ticket_guard = inner_lock.read();
    let ticket = ticket_guard.await;

    let response_body = GetTicketResponse(ticket.clone());

    let mut response = Response::new(StatusCode::Ok);
    response.set_body(Body::from_json(&response_body)?);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use crate::{listen, run_server, CreateTicketRequest, CreateTicketResponse, GetTicketResponse};
    use futures::future;
    use outro_08::data::{Status, Ticket};
    use outro_08::store::TicketId;
    use std::net::SocketAddr;
    use surf;
    use surf::Response;

    fn create_ticket_request(n: u64) -> CreateTicketRequest {
        CreateTicketRequest {
            title: format!("Title {}", n).to_string(),
            description: format!("Description {}", n).to_string(),
        }
    }

    async fn create_ticket(address: &SocketAddr, ticket_request: &CreateTicketRequest) -> Response {
        surf::post(format!("http://{}/tickets", address))
            .body_json(&ticket_request).unwrap().await.unwrap()
    }

    async fn get_ticket(address: &SocketAddr, ticket_id: TicketId) -> Response {
        let uri = format!("http://{}/tickets/{}", address, &ticket_id.0);
        surf::get(uri).await.unwrap()
    }


    #[tokio::test]
    async fn basic_server() {
        let listener = listen(None).await.unwrap();
        let address = listener.local_addr().unwrap().clone();
        let server = tokio::spawn(run_server(listener));

        let create_ticket_req = create_ticket_request(1);

        let mut response = create_ticket(&address, &create_ticket_req).await;

        assert_eq!(response.status(), 200);
        let response_body: CreateTicketResponse = response.body_json().await.unwrap();
        assert_eq!(response_body.ticket_id, 0.into());

        server.abort();
    }

    #[tokio::test]
    async fn multiple_tickets_are_properly_stored_and_can_be_retrieved() {
        let listener = listen(None).await.unwrap();
        let address = listener.local_addr().unwrap().clone();
        let server = tokio::spawn(run_server(listener));

        async fn create_and_get_ticket(address: &SocketAddr, n: u64) -> () {
            let new_ticket_req = create_ticket_request(n);
            let mut new_ticket_resp = create_ticket(address, &new_ticket_req).await;

            assert_eq!(new_ticket_resp.status(), 200);
            let ticket_id: TicketId = new_ticket_resp.body_json::<CreateTicketResponse>().await.unwrap().ticket_id;

            let mut get_ticket_resp = get_ticket(address, ticket_id).await;
            assert_eq!(get_ticket_resp.status(), 200);
            let retreived_ticket: Ticket = get_ticket_resp.body_json::<GetTicketResponse>().await.unwrap().0;

            assert_eq!(retreived_ticket.title.0, new_ticket_req.title);
            assert_eq!(retreived_ticket.description.0, new_ticket_req.description);
            assert_eq!(retreived_ticket.status, Status::ToDo);
            assert_eq!(retreived_ticket.id, ticket_id);

            ()
        }

        let requests = (1..3)
            .map(|i| create_and_get_ticket(&address, i))
            .collect::<Vec<_>>();

        future::join_all(requests).await;

        server.abort();
    }
}