use outro_08::data;
use outro_08::data::{Status, Ticket};
use outro_08::store::{TicketId, TicketStore};
use ticket_fields::{TicketDescription, TicketTitle};
use tide::prelude::*;
use tide::{Body, Request, Response, StatusCode};

#[derive(Clone, Debug, Serialize)]
#[serde(remote = "TicketTitle")]
pub struct TicketTitleSerializer(String);

#[derive(Clone, Debug, Serialize)]
#[serde(remote = "TicketDescription")]
pub struct TicketDescriptionSerializer(String);

#[derive(Clone, Debug, Serialize)]
#[serde(remote = "Ticket")]
pub struct TicketSerializer {
    pub id: TicketId,
    #[serde(with="TicketTitleSerializer")] pub title: TicketTitle,
    #[serde(with="TicketDescriptionSerializer")] pub description: TicketDescription,
    pub status: Status,
}

#[derive(Clone, Debug, Serialize)]
pub struct GetTicketResponse<'a>(#[serde(with="TicketSerializer")] &'a Ticket);

#[derive(Clone, Debug, Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
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
async fn main() -> tide::Result<()> {
    let store = TicketStore::new();
    let mut app = tide::with_state(store);
    app.at("/tickets").post(new_ticket);
    app.at("/tickets/:id").get(get_ticket);
    app.listen("127.0.0.1:8080").await?;
    Ok(())
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

    let response_body = GetTicketResponse(&ticket);

    let mut response = Response::new(StatusCode::Ok);
    response.set_body(Body::from_json(&response_body)?);
    Ok(response)
}