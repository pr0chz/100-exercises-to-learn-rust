use std::convert::TryInto;
use thiserror::Error;
use ticket_fields::{TicketDescription, TicketTitle};
use tide::prelude::*;
use tide::{Body, Request, Response, StatusCode};
use tokio::net::TcpListener;
use MyError::NotFound;
use crate::data::{Status, Ticket, TicketDraft};
use crate::server::MyError::BadRequest;
use crate::store::{TicketId, TicketStore};

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
pub struct GetTicketResponse(#[serde(with="TicketSerializer")] pub Ticket);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTicketRequest {
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTicketResponse {
    pub ticket_id: TicketId,
}

impl TryInto<TicketDraft> for CreateTicketRequest {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<TicketDraft, Self::Error> {
        let title = self.title.try_into()?;
        let description = self.description.try_into()?;
        let result = TicketDraft { title, description };
        Ok(result)
    }
}

pub async fn listen(port: Option<u16>) -> std::io::Result<TcpListener> {
    let bind_addr = format!("127.0.0.1:{}", port.unwrap_or(0));
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    println!("Server listening on {}", local_addr);
    Ok(listener)
}

#[derive(Error, Debug)]
pub enum MyError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

async fn error_handler(mut res: Response) -> tide::Result {
    if let Some(error) = res.downcast_error::<MyError>() {
        let status_code = match error {
            BadRequest(_) => { StatusCode::BadRequest }
            NotFound(_) => { StatusCode::NotFound }
        };
        let error_msg = error.to_string();
        res.set_status(status_code);
        res.set_body(error_msg);
    }
    Ok(res)
}

pub async fn run_server(listener: TcpListener) -> std::io::Result<()> {
    let store = TicketStore::new();
    let mut app = tide::with_state(store);
    app.with(tide::utils::After(error_handler));
    app.at("/tickets").post(new_ticket);
    app.at("/tickets/:id").get(get_ticket);
    // Use the listener for the Tide app
    app.listen(listener.into_std()?).await
}

pub async fn new_ticket(mut req: Request<TicketStore>) -> tide::Result {
    let ticket_request: CreateTicketRequest = req.body_json()
        .await.map_err(|_| BadRequest("Failed to parse.".to_string()))?;

    let store = req.state();
    let ticket_draft = ticket_request.try_into().map_err(|e| BadRequest(format!("Malformed ticket: {}", e)))?;
    let id: TicketId = store.write().await.add_ticket(ticket_draft);

    let response_body = CreateTicketResponse { ticket_id: id };

    let mut response = Response::new(StatusCode::Ok);
    response.set_body(Body::from_json(&response_body)?);
    Ok(response)
}

pub async fn get_ticket(req: Request<TicketStore>) -> tide::Result {
    let ticket_id = req
        .param("id").map_err(|_| BadRequest("Missing id parameter".to_string()))?
        .parse::<u64>().map_err(|_| BadRequest("Wrong id parameter".to_string()))?;

    let store = req.state();
    let inner_lock = store.read().await.get(TicketId(ticket_id)).ok_or(NotFound("Ticket not found".to_string()))?;
    let ticket_guard = inner_lock.read();
    let ticket = ticket_guard.await;

    let response_body = GetTicketResponse(ticket.clone());

    let mut response = Response::new(StatusCode::Ok);
    response.set_body(Body::from_json(&response_body)?);
    Ok(response)
}

